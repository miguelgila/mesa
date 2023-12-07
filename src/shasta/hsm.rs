/// Refs:
/// Member/node state --> https://apidocs.svc.cscs.ch/iaas/hardware-state-manager/overview/#section/Valid-State-Transistions
/// https://github.com/Cray-HPE/docs-csm/blob/release/1.3/api/smd.md
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
struct HsmGroup {
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    members: Option<Vec<Member>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Member {
    #[serde(skip_serializing_if = "Option::is_none")]
    ids: Option<Vec<String>>,
}

/* impl HsmGroup {
    pub fn new(
        label: String,
        description: Option<String>,
        tags: Option<Vec<String>>,
        members: Option<Vec<Member>>,
    ) -> Self {
        Self {
            label,
            description,
            tags,
            members,
        }
    }
} */

/* impl Member {
    pub fn new(ids: Option<Vec<String>>) -> Self {
        Self { ids }
    }
} */

pub mod http_client {

    use std::error::Error;

    use reqwest::Url;
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    /// https://github.com/Cray-HPE/docs-csm/blob/release/1.5/api/smd.md#post-groups
    pub async fn create_new_hsm_group(
        shasta_token: &str,
        shasta_base_url: &str,
        shasta_root_cert: &[u8],
        hsm_group_name_opt: &String, // label in HSM
        xnames: &Vec<String>,
        exclusive: &bool,
        description: &str,
        tags: &Vec<String>
    ) -> Result<Vec<Value>, Box<dyn Error>> {
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
        // Example body to create a new group:
        // {
        //   "label": "blue",
        //   "description": "This is the blue group",
        //   "tags": [
        //     "optional_tag1",
        //     "optional_tag2"
        //   ],
        //   "exclusiveGroup": "optional_excl_group",
        //   "members": {
        //     "ids": [
        //       "x1c0s1b0n0",
        //       "x1c0s1b0n1",
        //       "x1c0s2b0n0",
        //       "x1c0s2b0n1"
        //     ]
        //   }
        // }
        // Describe the JSON object
        #[derive(Serialize, Deserialize, Debug)]
        struct xname_array {
            ids: Vec<String>,
        }
        #[derive(Serialize, Deserialize, Debug)]
        struct hsm_group_json_body {
            label: String,
            description: String,
            // tags: Vec<String>,
            // exclusiveGroup: bool,
            members: xname_array,
        }
        // Create the variables that represent our JSON object
        let myxnames = xname_array {
            ids: xnames.clone(),
        };

        let hsm_group_json = hsm_group_json_body {
            label: hsm_group_name_opt.clone(),
            description: description.to_string().clone(),
            // tags: tags.clone(),
            // exclusiveGroup: exclusive.clone(),
            members: myxnames,
        };
        let hsm_group_json_body = match serde_json::to_string(&hsm_group_json) {
            Ok(m) => m,
            Err(e) => panic!("crap"),
        };

        println!("{:#?}", &hsm_group_json_body);

        // Ok(())
        // Some JSON input data as a &str. Maybe this comes from the user.

        let url_api = shasta_base_url.to_owned() + "/smd/hsm/v2/groups";

        let resp = client
            .post(url_api)
            .header("Authorization", format!("Bearer {}", shasta_token))
            .json(&hsm_group_json) // make sure this is not a string!
            .send()
            .await?;

        let json_response:Value;

        if resp.status().is_success() {
            json_response = serde_json::from_str(&resp.text().await?)?;
        } else {
            return Err(resp.text().await?.into()); // Black magic conversion from Err(Box::new("my error msg")) which does not
        };

        Ok(json_response.as_array().unwrap().to_owned())

    }
    pub async fn get_all_hsm_groups(
        shasta_token: &str,
        shasta_base_url: &str,
        shasta_root_cert: &[u8],
    ) -> Result<Vec<Value>, Box<dyn Error>> {
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

        let json_response: Value;

        let url_api = shasta_base_url.to_owned() + "/smd/hsm/v2/groups";

        let resp = client
            .get(url_api)
            .header("Authorization", format!("Bearer {}", shasta_token))
            .send()
            .await?;

        if resp.status().is_success() {
            json_response = serde_json::from_str(&resp.text().await?)?;
        } else {
            return Err(resp.text().await?.into()); // Black magic conversion from Err(Box::new("my error msg")) which does not
        };

        Ok(json_response.as_array().unwrap().to_owned())
    }

    /// Get list of HSM groups using --> https://apidocs.svc.cscs.ch/iaas/hardware-state-manager/operation/doGroupsGet/
    /// NOTE: this returns all HSM groups which name contains hsm_groupu_name param value
    pub async fn get_hsm_group_vec(
        shasta_token: &str,
        shasta_base_url: &str,
        shasta_root_cert: &[u8],
        hsm_group_name_opt: Option<&String>,
    ) -> Result<Vec<Value>, Box<dyn Error>> {
        let json_response =
            get_all_hsm_groups(shasta_token, shasta_base_url, shasta_root_cert).await?;

        let mut hsm_groups: Vec<Value> = Vec::new();

        if let Some(hsm_group_name) = hsm_group_name_opt {
            for hsm_group in json_response {
                if hsm_group["label"]
                    .as_str()
                    .unwrap()
                    .contains(hsm_group_name)
                {
                    hsm_groups.push(hsm_group.clone());
                }
            }
        }

        Ok(hsm_groups)
    }

    /// Get list of HSM group using --> shttps://apidocs.svc.cscs.ch/iaas/hardware-state-manager/operation/doGroupsGet/
    pub async fn get_hsm_group(
        shasta_token: &str,
        shasta_base_url: &str,
        shasta_root_cert: &[u8],
        hsm_group_name: &str,
    ) -> Result<Value, Box<dyn Error>> {
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

        let url_api = shasta_base_url.to_owned() + "/smd/hsm/v2/groups/" + hsm_group_name;

        let resp = client
            .get(url_api)
            .header("Authorization", format!("Bearer {}", shasta_token))
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(resp.json().await?)
            //json_response = serde_json::from_str(&resp.text().await?)?;
        } else {
            Err(resp.text().await?.into()) // Black magic conversion from Err(Box::new("my error msg")) which does not
        }
    }

    /// Fetches node/compnent details using HSM v2 ref --> https://apidocs.svc.cscs.ch/iaas/hardware-state-manager/operation/doComponentsGet/
    pub async fn get_component_status(
        shasta_token: &str,
        shasta_base_url: &str,
        shasta_root_cert: &[u8],
        xname: &str,
    ) -> Result<Value, Box<dyn Error>> {
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

        let resp = client
            .get(format!(
                "{}/smd/hsm/v2/State/Components/{}",
                shasta_base_url, xname
            ))
            .header("Authorization", format!("Bearer {}", shasta_token))
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(serde_json::from_str(&resp.text().await?)?)
        } else {
            Err(resp.json::<Value>().await?["detail"]
                .as_str()
                .unwrap()
                .into()) // Black magic conversion from Err(Box::new("my error msg")) which does not
        }
    }

    /// Fetches nodes/compnents details using HSM v2 ref --> https://apidocs.svc.cscs.ch/iaas/hardware-state-manager/operation/doComponentsGet/
    pub async fn get_components_status(
        shasta_token: &str,
        shasta_base_url: &str,
        shasta_root_cert: &[u8],
        xnames: Vec<String>,
    ) -> Result<Value, Box<dyn Error>> {
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

        let url_params: Vec<_> = xnames.iter().map(|xname| ("id", xname)).collect();
        let api_url = Url::parse_with_params(
            &format!("{}/smd/hsm/v2/State/Components", shasta_base_url),
            &url_params,
        )?;

        let resp = client
            .get(api_url)
            .header("Authorization", format!("Bearer {}", shasta_token))
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(serde_json::from_str(&resp.text().await?)?)
        } else {
            Err(resp.json::<Value>().await?["detail"]
                .as_str()
                .unwrap()
                .into()) // Black magic conversion from Err(Box::new("my error msg")) which does not
        }
    }

    pub async fn get_hw_inventory(
        shasta_token: &str,
        shasta_base_url: &str,
        shasta_root_cert: &[u8],
        xname: &str,
    ) -> Result<Value, Box<dyn Error>> {
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

        let api_url = format!(
            "{}/smd/hsm/v2/Inventory/Hardware/Query/{}",
            shasta_base_url, xname
        );

        let resp = client
            .get(api_url)
            .header("Authorization", format!("Bearer {}", shasta_token))
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(serde_json::from_str(&resp.text().await?)?)
        } else {
            Err(resp.json::<Value>().await?["detail"]
                .as_str()
                .unwrap()
                .into()) // Black magic conversion from Err(Box::new("my error msg")) which does not
        }
    }
}

pub mod utils {

    use std::collections::{HashMap, HashSet};

    use serde_json::{json, Value};

    use crate::shasta::hsm::http_client::get_all_hsm_groups;

    use super::http_client;

    pub fn get_member_vec_from_hsm_group_value(hsm_group: &Value) -> Vec<String> {
        // Take all nodes for all hsm_groups found and put them in a Vec
        hsm_group["members"]["ids"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .map(|xname| xname.as_str().unwrap().to_string())
            .collect()
    }

    /* pub fn get_member_vec_from_hsm_name_vec(
        shasta_token: &str,
        shasta_base_url: &str,
        shasta_root_cert: &[u8],
        hsm_name_vec: &Vec<String>,
    ) -> Vec<String> {
        let hsm_value_vec = get_all_hsm_groups(shasta_token, shasta_base_url, shasta_root_cert);
        get_member_vec_from_hsm_name_vec(
            shasta_token,
            shasta_base_url,
            shasta_root_cert,
            hsm_name_vec,
        )
    } */

    pub async fn get_member_vec_from_hsm_name_vec(
        shasta_token: &str,
        shasta_base_url: &str,
        shasta_root_cert: &[u8],
        hsm_name_vec: &Vec<String>,
    ) -> Vec<String> {
        let mut hsm_group_value_vec =
            get_all_hsm_groups(shasta_token, shasta_base_url, shasta_root_cert)
                .await
                .unwrap();

        hsm_group_value_vec.retain(|hsm_value| {
            hsm_name_vec.contains(&hsm_value["label"].as_str().unwrap().to_string())
        });

        Vec::from_iter(
            get_member_vec_from_hsm_group_value_vec(&hsm_group_value_vec)
                .iter()
                .cloned(),
        )
    }

    pub fn get_member_vec_from_hsm_group_value_vec(hsm_groups: &[Value]) -> HashSet<String> {
        hsm_groups
            .iter()
            .flat_map(get_member_vec_from_hsm_group_value)
            .collect()
    }

    /// Returns a Map with nodes and the list of hsm groups that node belongs to.
    /// eg "x1500b5c1n3 --> [ psi-dev, psi-dev_cn ]"
    pub fn group_members_by_hsm_group_from_hsm_groups_value(
        hsm_groups: &Vec<Value>,
    ) -> HashMap<String, Vec<String>> {
        let mut member_hsm_map: HashMap<String, Vec<String>> = HashMap::new();
        for hsm_group_value in hsm_groups {
            let hsm_group_name = hsm_group_value["label"].as_str().unwrap().to_string();
            for member in get_member_vec_from_hsm_group_value(hsm_group_value) {
                member_hsm_map
                    .entry(member)
                    .and_modify(|hsm_groups| hsm_groups.push(hsm_group_name.clone()))
                    .or_insert_with(|| vec![hsm_group_name.clone()]);
            }
        }

        member_hsm_map
    }

    pub async fn get_members_ids(
        shasta_token: &str,
        shasta_base_url: &str,
        shasta_root_cert: &[u8],
        hsm_group: &str,
    ) -> Vec<String> {
        // Take all nodes for all hsm_groups found and put them in a Vec
        http_client::get_hsm_group(shasta_token, shasta_base_url, shasta_root_cert, hsm_group)
            .await
            .unwrap()["members"]["ids"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .map(|xname| xname.as_str().unwrap().to_string())
            .collect()
    }

    pub async fn get_hsm_group_from_xname(
        shasta_token: &str,
        shasta_base_url: &str,
        shasta_root_cert: &[u8],
        xname: &String,
    ) -> Option<String> {
        let hsm_groups_details =
            get_all_hsm_groups(shasta_token, shasta_base_url, shasta_root_cert)
                .await
                .unwrap();

        for hsm_group_details in hsm_groups_details.iter() {
            if hsm_group_details["members"]["ids"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value.as_str().unwrap() == xname)
            {
                /* println!(
                    "hsm group seems to have the member:\n{:#?}",
                    hsm_group_details
                ); */
                return Some(hsm_group_details["label"].as_str().unwrap().to_string());
            }
        }

        None
    }

    /// This method will verify the HSM group in user config file and the HSM group the user is
    /// trying to access and it will verify if this access is granted.
    /// config_hsm_group is the HSM group name in manta config file (~/.config/manta/config) and
    /// hsm_group_accessed is the hsm group the user is trying to access (either trying to access a
    /// CFS session or in a SAT file.)
    pub async fn validate_config_hsm_group_and_hsm_group_accessed(
        shasta_token: &str,
        shasta_base_url: &str,
        shasta_root_cert: &[u8],
        hsm_group: Option<&String>,
        session_name: Option<&String>,
        cfs_sessions: &[Value],
    ) {
        if let Some(hsm_group_name) = hsm_group {
            let hsm_group_details = crate::shasta::hsm::http_client::get_hsm_group_vec(
                shasta_token,
                shasta_base_url,
                shasta_root_cert,
                hsm_group,
            )
            .await
            .unwrap();
            let hsm_group_members =
                crate::shasta::hsm::utils::get_member_vec_from_hsm_group_value_vec(
                    &hsm_group_details,
                );
            let cfs_session_hsm_groups: Vec<String> = cfs_sessions.last().unwrap()["target"]
                ["groups"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .map(|group| group["name"].as_str().unwrap().to_string())
                .collect();
            let cfs_session_members: Vec<String> = cfs_sessions.last().unwrap()["ansible"]["limit"]
                .as_str()
                .unwrap_or_default()
                .split(',')
                .map(|xname| xname.to_string())
                .collect();
            if !cfs_session_hsm_groups.contains(hsm_group_name)
                && !cfs_session_members
                    .iter()
                    .all(|cfs_session_member| hsm_group_members.contains(cfs_session_member))
            {
                println!(
                    "CFS session {} does not apply to HSM group {}",
                    session_name.unwrap(),
                    hsm_group_name
                );
                std::process::exit(1);
            }
            /* if !cfs_session_members
                .iter()
                .all(|cfs_session_member| hsm_group_members.contains(cfs_session_member))
            {
                println!(
                    "CFS session {} does not apply to HSM group {}",
                    session_name.unwrap(),
                    hsm_group_name
                );
                std::process::exit(1);
            } */
        }
    }

    pub fn get_list_processor_model_from_hw_inventory_value(
        hw_inventory: &Value,
    ) -> Option<Vec<String>> {
        hw_inventory["Nodes"].as_array().unwrap().first().unwrap()["Processors"]
            .as_array()
            .map(|processor_list: &Vec<Value>| {
                processor_list
                    .iter()
                    .map(|processor| {
                        processor
                            .pointer("/PopulatedFRU/ProcessorFRUInfo/Model")
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_string()
                    })
                    .collect::<Vec<String>>()
            })
    }

    pub fn get_list_accelerator_model_from_hw_inventory_value(
        hw_inventory: &Value,
    ) -> Option<Vec<String>> {
        hw_inventory["Nodes"].as_array().unwrap().first().unwrap()["NodeAccels"]
            .as_array()
            .map(|accelerator_list| {
                accelerator_list
                    .iter()
                    .map(|accelerator| {
                        accelerator
                            .pointer("/PopulatedFRU/NodeAccelFRUInfo/Model")
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_string()
                    })
                    .collect::<Vec<String>>()
            })
    }

    pub fn get_list_hsn_nics_model_from_hw_inventory_value(
        hw_inventory: &Value,
    ) -> Option<Vec<String>> {
        hw_inventory["Nodes"].as_array().unwrap().first().unwrap()["NodeHsnNics"]
            .as_array()
            .map(|hsn_nic_list| {
                hsn_nic_list
                    .iter()
                    .map(|hsn_nic| {
                        hsn_nic
                            .pointer("/NodeHsnNicLocationInfo/Description")
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_string()
                    })
                    .collect::<Vec<String>>()
            })
    }

    pub fn get_list_memory_capacity_from_hw_inventory_value(
        hw_inventory: &Value,
    ) -> Option<Vec<u64>> {
        hw_inventory["Nodes"].as_array().unwrap().first().unwrap()["Memory"]
            .as_array()
            .map(|memory_list| {
                memory_list
                    .iter()
                    .map(|memory| {
                        memory
                            .pointer("/PopulatedFRU/MemoryFRUInfo/CapacityMiB")
                            .unwrap_or(&json!(0))
                            .as_u64()
                            .unwrap()
                    })
                    .collect::<Vec<u64>>()
            })
    }
}
