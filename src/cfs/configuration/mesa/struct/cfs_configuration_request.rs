/// Structs related to CFS confguration with data related to most recent commit id like, author
/// name, commit date, etc
///
use std::path::PathBuf;

use k8s_openapi::chrono;
use serde::{Deserialize, Serialize};
use substring::Substring;

use crate::common::{gitea, local_git_repo};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Layer {
    #[serde(rename = "cloneUrl")]
    clone_url: String,
    #[serde(skip_serializing_if = "Option::is_none")] // Either commit or branch is passed
    commit: Option<String>,
    name: String,
    playbook: String,
    #[serde(skip_serializing_if = "Option::is_none")] // Either commit or branch is passed
    branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)] // TODO: investigate why serde can Deserialize dynamically syzed structs `Vec<Layer>`
pub struct CfsConfigurationRequest {
    pub name: String,
    pub layers: Vec<Layer>,
}

impl Layer {
    pub fn new(
        clone_url: String,
        commit: Option<String>,
        name: String,
        playbook: String,
        branch: Option<String>,
        tag: Option<String>,
    ) -> Self {
        Self {
            clone_url,
            commit,
            name,
            playbook,
            branch,
            tag,
        }
    }
}

impl Default for CfsConfigurationRequest {
    fn default() -> Self {
        Self::new()
    }
}

impl CfsConfigurationRequest {
    pub fn new() -> Self {
        Self {
            name: String::default(),
            layers: Vec::default(),
        }
    }

    pub fn add_layer(&mut self, layer: Layer) {
        self.layers.push(layer);
    }

    pub fn from_sat_file_serde_yaml(configuration_yaml: &serde_yaml::Value) -> Self {
        let mut cfs_configuration = Self::new();

        cfs_configuration.name = configuration_yaml["name"].as_str().unwrap().to_string();

        for layer_yaml in configuration_yaml["layers"].as_sequence().unwrap() {
            // println!("\n\n### Layer:\n{:#?}\n", layer_json);

            if layer_yaml.get("git").is_some() {
                // Git layer
                let repo_name = layer_yaml["name"].as_str().unwrap().to_string();
                let repo_url = layer_yaml["git"]["url"].as_str().unwrap().to_string();
                let layer = Layer::new(
                    repo_url,
                    layer_yaml["git"]["commit"]
                        .as_str()
                        .map(|commit| commit.to_string()),
                    repo_name,
                    layer_yaml["playbook"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    layer_yaml["git"]["branch"]
                        .as_str()
                        .map(|branch| branch.to_string()),
                    layer_yaml["git"]["tag"].as_str().map(|tag| tag.to_string()),
                );
                cfs_configuration.add_layer(layer);
            } else {
                // Product layer
                let repo_url = format!(
                    "https://api-gw-service-nmn.local/vcs/cray/{}-config-management.git",
                    layer_yaml["name"].as_str().unwrap()
                );
                let layer = Layer::new(
                    repo_url,
                    // Some(layer_json["product"]["commit"].as_str().unwrap_or_default().to_string()),
                    None,
                    layer_yaml["product"]["name"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    layer_yaml["playbook"].as_str().unwrap().to_string(),
                    Some(
                        layer_yaml["product"]["branch"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                    ),
                    None,
                );
                cfs_configuration.add_layer(layer);
            }
        }
        cfs_configuration
    }

    pub async fn create_from_repos(
        gitea_token: &str,
        gitea_base_url: &str,
        shasta_root_cert: &[u8],
        repos: Vec<PathBuf>,
        cfs_configuration_name: &String,
    ) -> Self {
        // Create CFS configuration
        let mut cfs_configuration = CfsConfigurationRequest::new();
        cfs_configuration.name = cfs_configuration_name.to_string();

        for repo_path in &repos {
            // Get repo from path
            let repo = match local_git_repo::get_repo(&repo_path.to_string_lossy()) {
                Ok(repo) => repo,
                Err(_) => {
                    eprintln!(
                        "Could not find a git repo in {}",
                        repo_path.to_string_lossy()
                    );
                    std::process::exit(1);
                }
            };

            // Get last (most recent) commit
            let local_last_commit = local_git_repo::get_last_commit(&repo).unwrap();

            // Get repo name
            let repo_ref_origin = repo.find_remote("origin").unwrap();

            log::info!("Repo ref origin URL: {}", repo_ref_origin.url().unwrap());

            let repo_ref_origin_url = repo_ref_origin.url().unwrap();

            let repo_name = repo_ref_origin_url.substring(
                repo_ref_origin_url.rfind(|c| c == '/').unwrap() + 1, // repo name should not include URI '/' separator
                repo_ref_origin_url.len(), // repo_ref_origin_url.rfind(|c| c == '.').unwrap(),
            );

            let api_url = "cray/".to_owned() + repo_name;

            // Check if repo and local commit id exists in Shasta cvs
            let shasta_commitid_details_resp = gitea::http_client::get_commit_details(
                &api_url,
                // &format!("/cray/{}", repo_name),
                &local_last_commit.id().to_string(),
                gitea_token,
                shasta_root_cert,
            )
            .await;

            // Check sync status between user face and shasta VCS
            let shasta_commitid_details: serde_json::Value = match shasta_commitid_details_resp {
                Ok(_) => {
                    log::debug!(
                        "Local latest commit id {} for repo {} exists in shasta",
                        local_last_commit.id(),
                        repo_name
                    );
                    shasta_commitid_details_resp.unwrap()
                }
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            };

            let clone_url = gitea_base_url.to_owned() + "/cray/" + repo_name;

            // Create CFS layer
            let cfs_layer = Layer::new(
                clone_url,
                Some(shasta_commitid_details["sha"].as_str().unwrap().to_string()),
                format!(
                    "{}-{}",
                    repo_name.substring(0, repo_name.len()),
                    chrono::offset::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
                ),
                String::from("site.yml"),
                None,
                None,
            );

            CfsConfigurationRequest::add_layer(&mut cfs_configuration, cfs_layer);
        }

        cfs_configuration
    }
}
