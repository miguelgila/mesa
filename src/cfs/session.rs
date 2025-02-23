pub mod shasta {

    pub mod http_client {

        use crate::cfs::session::mesa::r#struct::CfsSessionPostRequest;

        use serde_json::Value;
        use std::error::Error;

        /// Fetch CFS sessions ref --> https://apidocs.svc.cscs.ch/paas/cfs/operation/get_sessions/
        pub async fn get(
            shasta_token: &str,
            shasta_base_url: &str,
            shasta_root_cert: &[u8],
            session_name_opt: Option<&String>,
            is_succeded_opt: Option<bool>,
        ) -> Result<reqwest::Response, reqwest::Error> {
            let client_builder = reqwest::Client::builder()
                .add_root_certificate(reqwest::Certificate::from_pem(shasta_root_cert)?);

            // Build client
            let client = if let Ok(socks5_env) = std::env::var("SOCKS5") {
                // socks5 proxy
                log::debug!("SOCKS5 enabled");
                let socks5proxy = reqwest::Proxy::all(socks5_env)?;

                // rest client to authenticate
                client_builder.proxy(socks5proxy).build()?
            } else {
                client_builder.build()?
            };

            let api_url: String = if let Some(session_name) = session_name_opt {
                shasta_base_url.to_owned() + "/cfs/v2/sessions/" + session_name
            } else {
                shasta_base_url.to_owned() + "/cfs/v2/sessions"
            };

            // Add params to request
            let mut request_payload = Vec::new();

            if let Some(is_succeded) = is_succeded_opt {
                request_payload.push(("succeced", is_succeded));
            }

            let response_rslt = client
                .get(api_url)
                .query(&request_payload)
                .bearer_auth(shasta_token)
                .send()
                .await;

            match response_rslt {
                Ok(response) => response.error_for_status(),
                Err(error) => Err(error),
            }
        }

        pub async fn post(
            shasta_token: &str,
            shasta_base_url: &str,
            shasta_root_cert: &[u8],
            session: &CfsSessionPostRequest,
        ) -> Result<reqwest::Response, reqwest::Error> {
            log::debug!(
                "Session:\n{}",
                serde_json::to_string_pretty(session).unwrap()
            );

            let client_builder = reqwest::Client::builder()
                .add_root_certificate(reqwest::Certificate::from_pem(shasta_root_cert)?);

            // Build client
            let client = if let Ok(socks5_env) = std::env::var("SOCKS5") {
                // socks5 proxy
                log::debug!("SOCKS5 enabled");
                let socks5proxy = reqwest::Proxy::all(socks5_env)?;

                // rest client to authenticate
                client_builder.proxy(socks5proxy).build()?
            } else {
                client_builder.build()?
            };

            let api_url = shasta_base_url.to_owned() + "/cfs/v2/sessions";

            let response_rslt = client
                .post(api_url)
                // .post(format!("{}{}", shasta_base_url, "/cfs/v2/sessions"))
                .bearer_auth(shasta_token)
                .json(&session)
                .send()
                .await;

            match response_rslt {
                Ok(response) => Ok(response),
                Err(error) => Err(error),
            }
        }

        pub async fn delete(
            shasta_token: &str,
            shasta_base_url: &str,
            shasta_root_cert: &[u8],
            session_name: &str,
        ) -> Result<(), Box<dyn Error>> {
            log::info!("Deleting CFS session id: {}", session_name);

            let client;

            let client_builder = reqwest::Client::builder()
                .add_root_certificate(reqwest::Certificate::from_pem(shasta_root_cert)?);

            // Build client
            if std::env::var("SOCKS5").is_ok() {
                // socks5 proxy
                log::debug!("SOCKS5 enabled");
                let socks5proxy = reqwest::Proxy::all(std::env::var("SOCKS5").unwrap())?;

                // rest client to authenticate
                client = client_builder.proxy(socks5proxy).build()?;
            } else {
                client = client_builder.build()?;
            }

            let api_url = shasta_base_url.to_owned() + "/cfs/v2/sessions/" + session_name;

            let resp = client
                .delete(api_url)
                .bearer_auth(shasta_token)
                .send()
                .await?;

            if resp.status().is_success() {
                log::debug!("{:#?}", resp);
                Ok(())
            } else {
                log::debug!("{:#?}", resp);
                Err(resp.json::<Value>().await?["detail"]
                    .as_str()
                    .unwrap()
                    .into()) // Black magic conversion from Err(Box::new("my error msg")) which does not
            }
        }
    }

    pub mod utils {

        use std::collections::HashSet;

        use serde_json::Value;

        use crate::hsm;

        /// Fetch CFS sessions ref --> https://apidocs.svc.cscs.ch/paas/cfs/operation/get_sessions/
        /// Returns list of CFS sessions filtered by HSM group ordered by start time
        pub async fn filter_by_hsm(
            shasta_token: &str,
            shasta_base_url: &str,
            shasta_root_cert: &[u8],
            cluster_cfs_sessions: &mut Vec<Value>,
            hsm_group_name_vec: &[String],
            limit_number_opt: Option<&u8>,
        ) {
            let hsm_group_member_vec = hsm::group::shasta::utils::get_member_vec_from_hsm_name_vec(
                shasta_token,
                shasta_base_url,
                shasta_root_cert,
                hsm_group_name_vec,
            )
            .await;

            // Checks either target.groups contains hsm_group_name or ansible.limit is a subset of
            // hsm_group.members.ids
            cluster_cfs_sessions.retain(|cfs_session| {
                cfs_session["target"]["groups"]
                    .as_array()
                    .unwrap_or(&Vec::new())
                    .iter()
                    .any(|group| {
                        hsm_group_name_vec.contains(&group["name"].as_str().unwrap().to_string())
                    })
                    || cfs_session["ansible"]["limit"]
                        .as_str()
                        .unwrap_or("")
                        .split(',')
                        .map(|node| node.trim().to_string())
                        .collect::<HashSet<_>>()
                        .is_subset(&HashSet::from_iter(hsm_group_member_vec.clone()))
            });

            // Sort CFS sessions by start time order ASC
            cluster_cfs_sessions.sort_by(|a, b| {
                a["status"]["session"]["startTime"]
                    .as_str()
                    .unwrap()
                    .cmp(b["status"]["session"]["startTime"].as_str().unwrap())
            });

            if let Some(limit_number) = limit_number_opt {
                // Limiting the number of results to return to client

                *cluster_cfs_sessions = cluster_cfs_sessions[cluster_cfs_sessions
                    .len()
                    .saturating_sub(*limit_number as usize)..]
                    .to_vec();
            }
        }

        /// Fetch CFS sessions ref --> https://apidocs.svc.cscs.ch/paas/cfs/operation/get_sessions/
        /// Returns list of CFS sessions ordered by start time
        pub async fn filter(
            shasta_token: &str,
            shasta_base_url: &str,
            shasta_root_cert: &[u8],
            cluster_cfs_sessions: &mut Vec<Value>,
            hsm_group_name_vec: &[String],
            limit_number_opt: Option<&u8>,
        ) {
            let hsm_group_member_vec = hsm::group::shasta::utils::get_member_vec_from_hsm_name_vec(
                shasta_token,
                shasta_base_url,
                shasta_root_cert,
                hsm_group_name_vec,
            )
            .await;

            // Checks either target.groups contains hsm_group_name or ansible.limit is a subset of
            // hsm_group.members.ids
            cluster_cfs_sessions.retain(|cfs_session| {
                cfs_session["target"]["groups"]
                    .as_array()
                    .unwrap_or(&Vec::new())
                    .iter()
                    .any(|group| {
                        hsm_group_name_vec.contains(&group["name"].as_str().unwrap().to_string())
                    })
                    || cfs_session["ansible"]["limit"]
                        .as_str()
                        .unwrap_or("")
                        .split(',')
                        .map(|node| node.trim().to_string())
                        .collect::<HashSet<_>>()
                        .is_subset(&HashSet::from_iter(hsm_group_member_vec.clone()))
            });

            // Sort CFS sessions by start time order ASC
            cluster_cfs_sessions.sort_by(|a, b| {
                a["status"]["session"]["startTime"]
                    .as_str()
                    .unwrap()
                    .cmp(b["status"]["session"]["startTime"].as_str().unwrap())
            });

            if let Some(limit_number) = limit_number_opt {
                // Limiting the number of results to return to client

                *cluster_cfs_sessions = cluster_cfs_sessions[cluster_cfs_sessions
                    .len()
                    .saturating_sub(*limit_number as usize)..]
                    .to_vec();
            }
        }

        pub fn get_image_id_cfs_configuration_target_tuple_vec(
            cfs_session_value_vec: Vec<Value>,
        ) -> Vec<(String, String, Vec<String>)> {
            let mut image_id_cfs_configuration_target_from_cfs_session: Vec<(
                String,
                String,
                Vec<String>,
            )> = Vec::new();

            cfs_session_value_vec.iter().for_each(|cfs_session| {
                if let Some(result_id) = cfs_session.pointer("/status/artifacts/0/result_id") {
                    let target: Vec<String> =
                        if let Some(target_groups) = cfs_session.pointer("/target/groups") {
                            target_groups
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|group| group["name"].as_str().unwrap().to_string())
                                .collect()
                        } else if let Some(ansible_limit) = cfs_session.pointer("/ansible/limit") {
                            ansible_limit
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|xname| xname.as_str().unwrap().to_string())
                                .collect()
                        } else {
                            vec![]
                        };

                    image_id_cfs_configuration_target_from_cfs_session.push((
                        result_id.as_str().unwrap().to_string(),
                        cfs_session
                            .pointer("/configuration/name")
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_string(),
                        target,
                    ));
                } else {
                    image_id_cfs_configuration_target_from_cfs_session.push((
                        "".to_string(),
                        "".to_string(),
                        vec![],
                    ));
                }
            });

            image_id_cfs_configuration_target_from_cfs_session
        }

        pub fn get_image_id_from_cfs_session_vec(cfs_session_value_vec: &[Value]) -> Vec<String> {
            cfs_session_value_vec
                .iter()
                .filter(|cfs_session| {
                    cfs_session
                        .pointer("/target/definition")
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .eq("image")
                        && cfs_session
                            .pointer("/status/session/succeeded")
                            .unwrap_or(&serde_json::json!("false"))
                            .as_str()
                            .unwrap()
                            .eq("true")
                        && cfs_session
                            .pointer("/status/artifacts/0/result_id")
                            .is_some()
                })
                .map(|cfs_session| {
                    cfs_session
                        .pointer("/status/artifacts/0/result_id")
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_string()
                })
                .collect::<Vec<String>>()
        }
    }
}

pub mod mesa {
    pub mod r#struct {

        use std::collections::HashMap;

        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize, Clone)]
        pub struct CfsSessionGetResponse {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub name: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub configuration: Option<Configuration>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub ansible: Option<Ansible>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub target: Option<Target>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub status: Option<Status>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub tags: Option<HashMap<String, String>>,
        }

        impl CfsSessionGetResponse {
            /// Returns list of result_ids
            pub fn get_result_id(&self) -> Option<String> {
                self.status.as_ref().and_then(|status| {
                    status.artifacts.as_ref().and_then(|artifacts| {
                        artifacts
                            .first()
                            .and_then(|artifact| artifact.result_id.clone())
                    })
                })
            }

            /// Returns list of HSM groups targeted
            pub fn get_target_hsm(&self) -> Option<Vec<String>> {
                self.target.as_ref().and_then(|target| {
                    target
                        .groups
                        .as_ref()
                        .map(|group_vec| group_vec.iter().map(|group| group.name.clone()).collect())
                })
            }

            /// Returns list of xnames targeted
            pub fn get_target_xname(&self) -> Option<Vec<String>> {
                self.ansible.as_ref().and_then(|ansible| {
                    ansible.limit.as_ref().map(|limit| {
                        limit
                            .split(',')
                            .map(|xname| xname.trim().to_string())
                            .collect()
                    })
                })
            }

            /// Returns 'true' if the CFS session target definition is 'image'. Otherwise (target
            /// definiton dynamic) will return 'false'
            pub fn is_target_def_image(&self) -> bool {
                self.get_target_def()
                    .is_some_and(|target_def| target_def == "image")
            }

            /// Returns target definition of the CFS session:
            /// image --> CFS session to build an image
            /// dynamic --> CFS session to configure a node
            pub fn get_target_def(&self) -> Option<String> {
                self.target
                    .as_ref()
                    .and_then(|target| target.definition.clone())
            }

            pub fn get_configuration_name(&self) -> Option<String> {
                self.configuration
                    .as_ref()
                    .and_then(|configuration| configuration.name.clone())
            }

            pub fn is_success(&self) -> bool {
                self.status
                    .as_ref()
                    .unwrap()
                    .session
                    .as_ref()
                    .unwrap()
                    .succeeded
                    .as_ref()
                    .unwrap()
                    == "true"
            }
        }

        #[derive(Debug, Serialize, Deserialize, Clone)]
        pub struct Configuration {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub name: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub limit: Option<String>,
        }

        #[derive(Debug, Serialize, Deserialize, Clone)]
        pub struct Ansible {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub config: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub limit: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub verbosity: Option<u64>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub passthrough: Option<String>,
        }

        #[derive(Debug, Serialize, Deserialize, Clone)]
        pub struct Status {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub artifacts: Option<Vec<Artifact>>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub session: Option<Session>,
        }

        #[derive(Debug, Serialize, Deserialize, Clone)]
        pub struct Artifact {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub image_id: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub result_id: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub r#type: Option<String>,
        }

        #[derive(Debug, Serialize, Deserialize, Clone)]
        pub struct Session {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub job: Option<String>,
            #[serde(rename = "completionTime")]
            #[serde(skip_serializing_if = "Option::is_none")]
            pub completion_time: Option<String>,
            #[serde(rename = "startTime")]
            #[serde(skip_serializing_if = "Option::is_none")]
            pub start_time: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub status: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub succeeded: Option<String>,
        }

        #[derive(Debug, Serialize, Deserialize, Clone, Default)]
        pub struct CfsSessionPostRequest {
            pub name: String,
            #[serde(rename = "configurationName")]
            pub configuration_name: String,
            #[serde(rename = "configurationLimit")]
            #[serde(skip_serializing_if = "Option::is_none")]
            pub configuration_limit: Option<String>,
            #[serde(rename = "ansibleLimit")]
            #[serde(skip_serializing_if = "Option::is_none")]
            pub ansible_limit: Option<String>,
            #[serde(rename = "ansibleConfig")]
            #[serde(skip_serializing_if = "Option::is_none")]
            pub ansible_config: Option<String>,
            #[serde(rename = "ansibleVerbosity")]
            #[serde(skip_serializing_if = "Option::is_none")]
            pub ansible_verbosity: Option<u8>,
            #[serde(rename = "ansiblePassthrough")]
            #[serde(skip_serializing_if = "Option::is_none")]
            pub ansible_passthrough: Option<String>,
            #[serde(default)]
            pub target: Target,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub tags: Option<HashMap<String, String>>,
        }

        #[derive(Debug, Serialize, Deserialize, Clone)]
        pub struct Group {
            pub name: String,
            pub members: Vec<String>,
        }

        #[derive(Debug, Serialize, Deserialize, Clone, Default)]
        pub struct Target {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub definition: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub groups: Option<Vec<Group>>,
        }

        impl CfsSessionPostRequest {
            pub fn new(
                name: String,
                configuration_name: String,
                ansible_limit: Option<String>,
                ansible_verbosity: Option<u8>,
                ansible_passthrough: Option<String>,
                is_target_definition_image: bool,
                groups_name: Option<Vec<String>>,
                base_image_id: Option<String>,
            ) -> Self {
                // This code is fine... the fact that I put Self behind a variable is ok, since image param
                // is not a default param, then doing things differently is not an issue. I checked with
                // other Rust developers in their discord https://discord.com/channels/442252698964721669/448238009733742612/1081686300182188207
                let mut cfs_session = Self {
                    name,
                    configuration_name,
                    ansible_limit,
                    ansible_verbosity,
                    ansible_passthrough,
                    ..Default::default()
                };

                if is_target_definition_image {
                    let target_groups: Vec<Group> = groups_name
                        .unwrap()
                        .into_iter()
                        .map(|group_name| Group {
                            name: group_name,
                            members: vec![base_image_id.as_ref().unwrap().to_string()],
                        })
                        .collect();

                    cfs_session.target.definition = Some("image".to_string());
                    cfs_session.target.groups = Some(target_groups);
                }

                cfs_session
            }

            pub fn from_sat_file_serde_yaml(session_yaml: &serde_yaml::Value) -> Self {
                let groups_name = session_yaml["configuration_group_names"]
                    .as_sequence()
                    .unwrap()
                    .iter()
                    .map(|group_name| group_name.as_str().unwrap().to_string())
                    .collect();

                let cfs_session = CfsSessionPostRequest::new(
                    session_yaml["name"].as_str().unwrap().to_string(),
                    session_yaml["configuration"].as_str().unwrap().to_string(),
                    None,
                    None,
                    None,
                    true,
                    Some(groups_name),
                    // Some(base_image_id.to_string()),
                    Some(session_yaml["ims"]["id"].as_str().unwrap().to_string()),
                );
                cfs_session
            }
        }

        /* #[derive(thiserror::Error, Debug)]
        pub enum CsmApiError {
            #[error("Error: {0}")]
            Error404(String),
            #[error("Crash: {0}")]
            ErrorCrash(serde_json::Value),
        } */

        #[derive(thiserror::Error, Debug)]
        pub enum ApiError {
            #[error("Error: {0}")]
            MesaError(String),
            #[error("Error: {0}")]
            CsmError(String),
            #[error("Crash: {0}")]
            ErrorCrash(serde_json::Value),
        }
    }

    pub mod http_client {

        use super::r#struct::{ApiError, CfsSessionGetResponse, CfsSessionPostRequest};

        /// Fetch CFS sessions ref --> https://apidocs.svc.cscs.ch/paas/cfs/operation/get_sessions/
        /// Returns list of CFS sessions ordered by start time.
        /// This methods filter by either HSM group name or HSM group members or both
        pub async fn get(
            shasta_token: &str,
            shasta_base_url: &str,
            shasta_root_cert: &[u8],
            session_name_opt: Option<&String>,
            is_succeded_opt: Option<bool>,
        ) -> Result<Vec<CfsSessionGetResponse>, reqwest::Error> {
            let response_rslt = crate::cfs::session::shasta::http_client::get(
                shasta_token,
                shasta_base_url,
                shasta_root_cert,
                session_name_opt,
                is_succeded_opt,
            )
            .await;

            let mut cfs_session_vec: Vec<CfsSessionGetResponse> = match response_rslt {
                Ok(response) => {
                    if session_name_opt.is_none() {
                        response.json::<Vec<CfsSessionGetResponse>>().await.unwrap()
                    } else {
                        vec![response.json::<CfsSessionGetResponse>().await.unwrap()]
                    }
                }
                Err(error) => return Err(error),
            };

            // Sort CFS sessions by start time order ASC
            cfs_session_vec.sort_by(|a, b| {
                a.status
                    .as_ref()
                    .unwrap()
                    .session
                    .as_ref()
                    .unwrap()
                    .start_time
                    .as_ref()
                    .unwrap()
                    .cmp(
                        b.status
                            .as_ref()
                            .unwrap()
                            .session
                            .as_ref()
                            .unwrap()
                            .start_time
                            .as_ref()
                            .unwrap(),
                    )
            });

            Ok(cfs_session_vec)
        }

        pub async fn post(
            shasta_token: &str,
            shasta_base_url: &str,
            shasta_root_cert: &[u8],
            session: &CfsSessionPostRequest,
        ) -> Result<CfsSessionGetResponse, ApiError> {
            let response = crate::cfs::session::shasta::http_client::post(
                shasta_token,
                shasta_base_url,
                shasta_root_cert,
                session,
            )
            .await
            .unwrap();

            if response.status().is_success() {
                Ok(response.json::<CfsSessionGetResponse>().await.unwrap())
            } else {
                let error_detail = response.json::<serde_json::Value>().await.unwrap()["detail"]
                    .as_str()
                    .unwrap()
                    .trim()
                    .to_string();
                Err(ApiError::CsmError(error_detail))
            }
        }
    }

    pub mod utils {
        use crate::hsm;

        use super::r#struct::CfsSessionGetResponse;

        /// Filter CFS sessions related to a list of HSM group names, how this works is, it will
        /// get the list of nodes within those HSM groups and filter all CFS sessions in the system
        /// using either the HSM group names or nodes as target.
        /// NOTE: Please make sure the user has access to the HSM groups he is asking for before
        /// calling this function
        pub async fn filter_by_hsm(
            shasta_token: &str,
            shasta_base_url: &str,
            shasta_root_cert: &[u8],
            cfs_session_vec: &mut Vec<CfsSessionGetResponse>,
            hsm_group_name_vec: &[String],
            limit_number_opt: Option<&u8>,
        ) {
            let xname_vec: Vec<String> =
                hsm::group::shasta::utils::get_member_vec_from_hsm_name_vec(
                    shasta_token,
                    shasta_base_url,
                    shasta_root_cert,
                    hsm_group_name_vec,
                )
                .await;

            // Checks either target.groups contains hsm_group_name or ansible.limit is a subset of
            // hsm_group.members.ids
            if !hsm_group_name_vec.is_empty() {
                cfs_session_vec.retain(|cfs_session| {
                    /* cfs_session.target.clone().is_some_and(|target| {
                        target.groups.is_some_and(|groups| {
                            !groups.is_empty()
                                && groups
                                    .iter()
                                    .any(|group| hsm_group_name_vec.contains(&group.name))
                        })
                    }) || cfs_session.ansible.clone().is_some_and(|ansible| {
                        ansible.limit.is_some_and(|limit| {
                            limit
                                .split(',')
                                .map(|node| node.trim().to_string())
                                .collect::<HashSet<String>>()
                                .is_subset(&HashSet::from_iter(xname_vec.clone()))
                        })
                    }) */
                    cfs_session.get_target_hsm().is_some_and(|target_hsm_vec| {
                        target_hsm_vec
                            .iter()
                            .any(|target_hsm| hsm_group_name_vec.contains(&target_hsm))
                    }) || cfs_session
                        .get_target_xname()
                        .is_some_and(|target_xname_vec| {
                            target_xname_vec
                                .iter()
                                .any(|target_xname| xname_vec.contains(target_xname))
                        })
                });

                if let Some(limit_number) = limit_number_opt {
                    // Limiting the number of results to return to client
                    *cfs_session_vec = cfs_session_vec
                        [cfs_session_vec.len().saturating_sub(*limit_number as usize)..]
                        .to_vec();
                }
            }
        }

        pub async fn filter_by_xname(
            shasta_token: &str,
            shasta_base_url: &str,
            shasta_root_cert: &[u8],
            cfs_session_vec: &mut Vec<CfsSessionGetResponse>,
            xname_vec: &[String],
            limit_number_opt: Option<&u8>,
        ) {
            let hsm_group_name_vec: Vec<String> =
                hsm::group::shasta::utils::get_hsm_group_vec_from_xname_vec(
                    shasta_token,
                    shasta_base_url,
                    shasta_root_cert,
                    xname_vec,
                )
                .await;

            // Checks either target.groups contains hsm_group_name or ansible.limit is a subset of
            // hsm_group.members.ids
            if !hsm_group_name_vec.is_empty() {
                cfs_session_vec.retain(|cfs_session| {
                    /* cfs_session.target.clone().is_some_and(|target| {
                        target.groups.is_some_and(|groups| {
                            !groups.is_empty()
                                && groups
                                    .iter()
                                    .any(|group| hsm_group_name_vec.contains(&group.name))
                        })
                    }) || cfs_session.ansible.clone().is_some_and(|ansible| {
                        ansible.limit.is_some_and(|limit| {
                            limit
                                .split(',')
                                .map(|node| node.trim().to_string())
                                .collect::<HashSet<String>>()
                                .is_subset(&HashSet::from_iter(xname_vec.to_vec()))
                        })
                    }) */
                    cfs_session.get_target_hsm().is_some_and(|target_hsm_vec| {
                        target_hsm_vec
                            .iter()
                            .any(|target_hsm| hsm_group_name_vec.contains(&target_hsm))
                    }) || cfs_session
                        .get_target_xname()
                        .is_some_and(|target_xname_vec| {
                            target_xname_vec
                                .iter()
                                .any(|target_xname| xname_vec.contains(target_xname))
                        })
                });

                if let Some(limit_number) = limit_number_opt {
                    // Limiting the number of results to return to client
                    *cfs_session_vec = cfs_session_vec
                        [cfs_session_vec.len().saturating_sub(*limit_number as usize)..]
                        .to_vec();
                }
            }
        }

        /// Filter CFS sessions related to a list of HSM group names and a list of nodes and filter
        /// all CFS sessions in the system using either the HSM group names or nodes as target.
        /// NOTE: Please make sure the user has access to the HSM groups and nodes he is asking for before
        /// calling this function
        pub fn find_cfs_session_related_to_image_id(
            cfs_session_vec: &[CfsSessionGetResponse],
            image_id: &str,
        ) -> Option<CfsSessionGetResponse> {
            /* cfs_session_vec
            .iter()
            .find(|cfs_session_value| {
                cfs_session_value.status.as_ref().is_some_and(|status| {
                    status.artifacts.as_ref().is_some_and(|artifact| {
                        artifact.first().as_ref().is_some_and(|first_artifact| {
                            first_artifact.result_id.as_ref().unwrap().eq(image_id)
                        })
                    })
                })
            })
            .cloned() */
            cfs_session_vec
                .iter()
                .find(|cfs_session| {
                    cfs_session
                        .get_result_id()
                        .is_some_and(|result_id| result_id == image_id)
                })
                .cloned()
        }

        pub fn get_cfs_configuration_name(cfs_session: &CfsSessionGetResponse) -> Option<String> {
            cfs_session
                .configuration
                .as_ref()
                .unwrap()
                .name
                .as_ref()
                .cloned()
        }

        /// Returns a tuple like (image_id, cfs_configuration_name, target) from a list of CFS
        /// sessions
        pub fn get_image_id_cfs_configuration_target_tuple_vec(
            cfs_session_vec: Vec<CfsSessionGetResponse>,
        ) -> Vec<(String, String, Vec<String>)> {
            let mut image_id_cfs_configuration_target_from_cfs_session: Vec<(
                String,
                String,
                Vec<String>,
            )> = Vec::new();

            cfs_session_vec.iter().for_each(|cfs_session| {
                /* let result_id: String = cfs_session
                .status
                .as_ref()
                .and_then(|status| {
                    status.artifacts.as_ref().and_then(|artifacts| {
                        artifacts
                            .first()
                            .and_then(|artifact| artifact.result_id.as_ref())
                    })
                })
                .unwrap_or(&"".to_string())
                .to_string(); */

                let result_id: String = cfs_session.get_result_id().unwrap_or("".to_string());

                /* let target: Vec<String> = if let Some(target_groups) =
                    cfs_session.target.as_ref().unwrap().groups.as_ref()
                {
                    target_groups
                        .iter()
                        .map(|group| group.name.clone())
                        .collect()
                } else if let Some(ansible_limit) =
                    cfs_session.ansible.as_ref().unwrap().limit.as_ref()
                {
                    ansible_limit
                        .split(',')
                        .map(|xname| xname.trim().to_string())
                        .collect()
                } else {
                    vec![]
                }; */

                let target: Vec<String> = cfs_session
                    .get_target_hsm()
                    .or_else(|| cfs_session.get_target_xname())
                    .unwrap_or(Vec::new());

                let cfs_configuration = cfs_session.get_configuration_name().unwrap();

                image_id_cfs_configuration_target_from_cfs_session.push((
                    result_id,
                    /* cfs_session
                    .configuration
                    .as_ref()
                    .unwrap()
                    .name
                    .as_ref()
                    .unwrap()
                    .to_string(), */
                    cfs_configuration,
                    target,
                ));
            });

            image_id_cfs_configuration_target_from_cfs_session
        }

        /// Returns a tuple like (image_id, cfs_configuration_name, target) from a list of CFS
        /// sessions. Only returns values from CFS sessions with an artifact.result_id value
        /// (meaning CFS sessions completed and successful of type image)
        pub fn get_image_id_cfs_configuration_target_for_existing_images_tuple_vec(
            cfs_session_vec: Vec<CfsSessionGetResponse>,
        ) -> Vec<(String, String, Vec<String>)> {
            let mut image_id_cfs_configuration_target_from_cfs_session: Vec<(
                String,
                String,
                Vec<String>,
            )> = Vec::new();

            cfs_session_vec.iter().for_each(|cfs_session| {
                if let Some(result_id) = cfs_session.get_result_id()
                /* .status
                .as_ref()
                .unwrap()
                .artifacts
                .as_ref()
                .and_then(|artifact_vec| {
                    artifact_vec
                        .first()
                        .and_then(|artifact| artifact.result_id.as_ref())
                }) */
                {
                    let target: Vec<String> = cfs_session
                        .get_target_hsm()
                        .or_else(|| cfs_session.get_target_xname())
                        .unwrap_or(Vec::new());

                    /* let target: Vec<String> = if let Some(target_groups) =
                        cfs_session.target.as_ref().unwrap().groups.as_ref()
                    {
                        target_groups
                            .iter()
                            .map(|group| group.name.clone())
                            .collect()
                    } else if let Some(ansible_limit) =
                        cfs_session.ansible.as_ref().unwrap().limit.as_ref()
                    {
                        ansible_limit
                            .split(',')
                            .map(|xname| xname.trim().to_string())
                            .collect()
                    } else {
                        vec![]
                    }; */

                    let cfs_configuration = cfs_session.get_configuration_name().unwrap();

                    image_id_cfs_configuration_target_from_cfs_session.push((
                        result_id.to_string(),
                        cfs_configuration,
                        /* cfs_session
                        .configuration
                        .as_ref()
                        .unwrap()
                        .name
                        .as_ref()
                        .unwrap()
                        .to_string(), */
                        target,
                    ));
                } else {
                    image_id_cfs_configuration_target_from_cfs_session.push((
                        "".to_string(),
                        "".to_string(),
                        vec![],
                    ));
                }
            });

            image_id_cfs_configuration_target_from_cfs_session
        }

        /// Return a list of the images ids related with a list of CFS sessions. The result list if
        /// filtered to CFS session completed and target def 'image' therefore the length of the
        /// resulting list may be smaller than the list of CFS sessions
        pub fn get_image_id_from_cfs_session_vec(
            cfs_session_value_vec: &[CfsSessionGetResponse],
        ) -> Vec<String> {
            cfs_session_value_vec
                .iter()
                .filter(|cfs_session| {
                    cfs_session.is_target_def_image()
                        /* .target
                        .as_ref()
                        .unwrap()
                        .definition
                        .as_ref()
                        .unwrap()
                        .eq("image") */
                        && cfs_session.is_success()
                            /* .status
                            .as_ref()
                            .unwrap()
                            .session
                            .as_ref()
                            .unwrap()
                            .succeeded
                            .as_ref()
                            .unwrap_or(&"false".to_string())
                            .eq("true") */
                        && cfs_session.get_result_id().is_some()
                    /* .status
                    .as_ref()
                    .unwrap()
                    .artifacts
                    .as_ref()
                    .unwrap()
                    .first()
                    .unwrap()
                    .result_id
                    .is_some() */
                })
                .map(|cfs_session| {
                    cfs_session.get_result_id().unwrap()
                    /* .status
                    .as_ref()
                    .unwrap()
                    .artifacts
                    .as_ref()
                    .unwrap()
                    .first()
                    .unwrap()
                    .result_id
                    .as_ref()
                    .unwrap()
                    .to_string() */
                })
                .collect::<Vec<String>>()
        }
    }
}

#[cfg(test)]
pub mod test {
    use crate::cfs::session::mesa::r#struct::CfsSessionGetResponse;

    #[tokio::test]
    async fn test_cfs_session_serde_json_to_struct_conversion() {
        let cfs_session_value = serde_json::json!({
          "ansible": {
            "config": "cfs-default-ansible-cfg",
            "limit": "x1005c1s2b0n0,x1005c0s3b0n0",
            "passthrough": null,
            "verbosity": 0
          },
          "configuration": {
            "limit": "",
            "name": "clariden-cos-config-2.3.110-96-3"
          },
          "name": "batcher-e5c059a8-20c1-4779-9c0b-a270ff081d63",
          "status": {
            "artifacts": [],
            "session": {
              "completionTime": "2023-10-10T08:46:34",
              "job": "cfs-298b9145-7504-4241-a985-7a2f301cdd9f",
              "startTime": "2023-10-10T08:36:40",
              "status": "complete",
              "succeeded": "true"
            }
          },
          "tags": {
            "bos_session": "d452344f-4aad-4747-bfcb-8d016b5524bc"
          },
          "target": {
            "definition": "dynamic",
            "groups": null
          }
        });

        let cfs_session = serde_json::from_value::<CfsSessionGetResponse>(cfs_session_value);

        println!("{:#?}", cfs_session);
    }
}
