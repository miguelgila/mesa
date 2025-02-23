/// Refs:
/// Member/node state --> https://apidocs.svc.cscs.ch/iaas/hardware-state-manager/overview/#section/Valid-State-Transistions

pub mod r#hw_components {
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use std::str::FromStr;
    use std::string::ToString;
    use strum_macros::{AsRefStr, Display, EnumIter, EnumString, IntoStaticStr};

    #[derive(
        Debug, EnumIter, EnumString, IntoStaticStr, AsRefStr, Display, Serialize, Deserialize, Clone,
    )]
    pub enum ArtifactType {
        Memory,
        Processor,
        NodeAccel,
        NodeHsnNic,
        Drive,
        CabinetPDU,
        CabinetPDUPowerConnector,
        CMMRectifier,
        NodeAccelRiser,
        NodeEnclosurePowerSupplie,
        NodeBMC,
        RouterBMC,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct NodeSummary {
        pub xname: String,
        pub r#type: String,
        pub processors: Vec<ArtifactSummary>,
        pub memory: Vec<ArtifactSummary>,
        pub node_accels: Vec<ArtifactSummary>,
        pub node_hsn_nics: Vec<ArtifactSummary>,
    }

    impl NodeSummary {
        pub fn from_csm_value(hw_artifact_value: Value) -> Self {
            let processors = hw_artifact_value["Processors"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .map(|processor_value| {
                    ArtifactSummary::from_processor_value(processor_value.clone())
                })
                .collect();

            let memory = hw_artifact_value["Memory"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .map(|memory_value| ArtifactSummary::from_memory_value(memory_value.clone()))
                .collect();

            let node_accels = hw_artifact_value["NodeAccels"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .map(|nodeaccel_value| {
                    ArtifactSummary::from_nodeaccel_value(nodeaccel_value.clone())
                })
                .collect();

            let node_hsn_nics = hw_artifact_value["NodeHsnNics"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .map(|nodehsnnic_value| {
                    ArtifactSummary::from_nodehsnnics_value(nodehsnnic_value.clone())
                })
                .collect();

            Self {
                xname: hw_artifact_value["ID"].as_str().unwrap().to_string(),
                r#type: hw_artifact_value["Type"].as_str().unwrap().to_string(),
                processors,
                memory,
                node_accels,
                node_hsn_nics,
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct ArtifactSummary {
        pub xname: String,
        pub r#type: ArtifactType,
        pub info: Option<String>,
    }

    impl ArtifactSummary {
        fn from_processor_value(processor_value: Value) -> Self {
            Self {
                xname: processor_value["ID"].as_str().unwrap().to_string(),
                r#type: ArtifactType::from_str(processor_value["Type"].as_str().unwrap()).unwrap(),
                info: processor_value
                    .pointer("/PopulatedFRU/ProcessorFRUInfo/Model")
                    .map(|model| model.as_str().unwrap().to_string()),
            }
        }

        fn from_memory_value(memory_value: Value) -> Self {
            // println!("DEBUG - memory raw data: {:#?}", memory_value);
            Self {
                xname: memory_value["ID"].as_str().unwrap().to_string(),
                r#type: ArtifactType::from_str(memory_value["Type"].as_str().unwrap()).unwrap(),
                info: memory_value
                    .pointer("/PopulatedFRU/MemoryFRUInfo/CapacityMiB")
                    .map(|capacity_mib| capacity_mib.as_number().unwrap().to_string() + " MiB"),
            }
        }

        fn from_nodehsnnics_value(nodehsnnic_value: Value) -> Self {
            Self {
                xname: nodehsnnic_value["ID"].as_str().unwrap().to_string(),
                r#type: ArtifactType::from_str(nodehsnnic_value["Type"].as_str().unwrap()).unwrap(),
                info: nodehsnnic_value
                    .pointer("/NodeHsnNicLocationInfo/Description")
                    .map(|description| description.as_str().unwrap().to_string()),
            }
        }

        fn from_nodeaccel_value(nodeaccel_value: Value) -> Self {
            Self {
                xname: nodeaccel_value["ID"].as_str().unwrap().to_string(),
                r#type: ArtifactType::from_str(nodeaccel_value["Type"].as_str().unwrap()).unwrap(),
                info: nodeaccel_value
                    .pointer("/PopulatedFRU/NodeAccelFRUInfo/Model")
                    .map(|model| model.as_str().unwrap().to_string()),
            }
        }
    }
}

pub mod r#struct {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct HsmGroup {
        pub label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub tags: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub members: Option<Member>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename(serialize = "exclusiveGroup"))]
        pub exclusive_group: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize, Default, Clone)]
    pub struct Member {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub ids: Option<Vec<String>>,
    }
}

pub mod group {
    pub mod shasta {
        pub mod http_client {

            use std::error::Error;

            use serde_json::Value;

            /// Get list of HSM group using --> shttps://apidocs.svc.cscs.ch/iaas/hardware-state-manager/operation/doGroupsGet/
            pub async fn get_raw(
                shasta_token: &str,
                shasta_base_url: &str,
                shasta_root_cert: &[u8],
                group_name_opt: Option<&String>,
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

                let api_url: String = if let Some(group_name) = group_name_opt {
                    shasta_base_url.to_owned() + "/smd/hsm/v2/groups/" + group_name
                } else {
                    shasta_base_url.to_owned() + "/smd/hsm/v2/groups"
                };

                let response_rslt = client.get(api_url).bearer_auth(shasta_token).send().await;

                match response_rslt {
                    Ok(response) => response.error_for_status(),
                    Err(error) => Err(error),
                }
            }

            pub async fn get(
                shasta_token: &str,
                shasta_base_url: &str,
                shasta_root_cert: &[u8],
                group_name_opt: Option<&String>,
            ) -> Result<Vec<Value>, reqwest::Error> {
                let response_rslt = get_raw(
                    shasta_token,
                    shasta_base_url,
                    shasta_root_cert,
                    group_name_opt,
                )
                .await;

                let hsm_group_value_vec: Vec<Value> = match response_rslt {
                    Ok(response) => {
                        if group_name_opt.is_none() {
                            response.json::<Vec<Value>>().await.unwrap()
                        } else {
                            vec![response.json::<Value>().await.unwrap()]
                        }
                    }
                    Err(error) => return Err(error),
                };

                Ok(hsm_group_value_vec)
            }

            pub async fn get_all(
                shasta_token: &str,
                shasta_base_url: &str,
                shasta_root_cert: &[u8],
            ) -> Result<Vec<Value>, reqwest::Error> {
                get(shasta_token, shasta_base_url, shasta_root_cert, None).await
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
                    get_all(shasta_token, shasta_base_url, shasta_root_cert).await?;

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
        }

        pub mod utils {

            use std::collections::{HashMap, HashSet};

            use serde_json::Value;

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

            /// Get the list of xnames which are members of a list of HSM groups. 
            /// eg: 
            /// given following HSM groups:
            /// tenant_a: [x1003c1s7b0n0, x1003c1s7b0n1]
            /// tenant_b: [x1003c1s7b1n0]
            /// Then calling this function with hsm_name_vec: &["tenant_a", "tenant_b"] should return [x1003c1s7b0n0, x1003c1s7b0n1, x1003c1s7b1n0]
            pub async fn get_member_vec_from_hsm_name_vec(
                shasta_token: &str,
                shasta_base_url: &str,
                shasta_root_cert: &[u8],
                hsm_name_vec: &[String],
            ) -> Vec<String> {
                let mut hsm_group_value_vec =
                    http_client::get_all(shasta_token, shasta_base_url, shasta_root_cert)
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

            pub fn get_member_vec_from_hsm_group_value_vec(
                hsm_groups: &[Value],
            ) -> HashSet<String> {
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

            pub async fn get_member_vec_from_hsm_group_name(
                shasta_token: &str,
                shasta_base_url: &str,
                shasta_root_cert: &[u8],
                hsm_group: &str,
            ) -> Vec<String> {
                // Take all nodes for all hsm_groups found and put them in a Vec
                http_client::get(
                    shasta_token,
                    shasta_base_url,
                    shasta_root_cert,
                    Some(&hsm_group.to_string()),
                )
                .await
                .unwrap()
                .first()
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
                let hsm_group_value_vec =
                    http_client::get_all(shasta_token, shasta_base_url, shasta_root_cert)
                        .await
                        .unwrap();

                for hsm_group_details in hsm_group_value_vec.iter() {
                    if hsm_group_details["members"]["ids"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|hsm_group_member| hsm_group_member.as_str().unwrap() == xname)
                    {
                        return Some(hsm_group_details["label"].as_str().unwrap().to_string());
                    }
                }

                None
            }

            /// Returns the list of HSM group names related to a list of nodes
            pub async fn get_hsm_group_vec_from_xname_vec(
                shasta_token: &str,
                shasta_base_url: &str,
                shasta_root_cert: &[u8],
                xname_vec: &[String],
            ) -> Vec<String> {
                let xname_value_vec: Vec<Value> = xname_vec
                    .iter()
                    .map(|xname| serde_json::json!(xname))
                    .collect();

                let mut hsm_group_value_vec =
                    http_client::get_all(shasta_token, shasta_base_url, shasta_root_cert)
                        .await
                        .unwrap();

                hsm_group_value_vec.retain(|hsm_group_value| {
                    hsm_group_value["members"]["ids"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|hsm_group_member| xname_value_vec.contains(hsm_group_member))
                });

                hsm_group_value_vec
                    .iter()
                    .map(|hsm_group_value| hsm_group_value["label"].as_str().unwrap().to_string())
                    .collect::<Vec<String>>()
            }

            pub fn get_hsm_group_from_cfs_session_related_to_cfs_configuration(
                cfs_session_value_vec: &[Value],
                cfs_configuration: &str,
            ) -> Vec<String> {
                let mut hsm_group_from_cfs_session_vec = cfs_session_value_vec
                    .iter()
                    .filter(|cfs_session| {
                        cfs_session
                            .pointer("/configuration/name")
                            .unwrap()
                            .eq(cfs_configuration)
                    })
                    .flat_map(|cfs_session| {
                        cfs_session
                            .pointer("/target/groups")
                            .unwrap()
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|group| group["name"].as_str().unwrap().to_string())
                    })
                    .collect::<Vec<String>>();

                hsm_group_from_cfs_session_vec.sort();
                hsm_group_from_cfs_session_vec.dedup();

                hsm_group_from_cfs_session_vec
            }

            pub fn get_hsm_group_from_bos_sessiontimplate_related_to_cfs_configuration(
                bos_sessiontemplate_value_vec: &[Value],
                cfs_configuration: &str,
            ) -> Vec<String> {
                let hsm_group_from_bos_sessiontemplate_computer_related_to_cfs_configuration =
                    bos_sessiontemplate_value_vec
                        .iter()
                        .filter(|bos_sessiontemplate| {
                            bos_sessiontemplate
                                .pointer("/cfs/configuration")
                                .unwrap()
                                .eq(cfs_configuration)
                        })
                        .flat_map(|bos_sessiontemplate| {
                            bos_sessiontemplate
                                .pointer("/boot_sets/compute/node_groups")
                                .unwrap()
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|node_group| node_group.as_str().unwrap().to_string())
                        });

                let hsm_group_from_bos_sessiontemplate_uan_related_to_cfs_configuration =
                    bos_sessiontemplate_value_vec
                        .iter()
                        .filter(|bos_sessiontemplate| {
                            bos_sessiontemplate
                                .pointer("/cfs/configuration")
                                .unwrap()
                                .eq(cfs_configuration)
                                && bos_sessiontemplate
                                    .pointer("/boot_sets/uan/node_groups")
                                    .is_some()
                        })
                        .flat_map(|bos_sessiontemplate| {
                            bos_sessiontemplate
                                .pointer("/boot_sets/uan/node_groups")
                                .unwrap()
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|node_group| node_group.as_str().unwrap().to_string())
                        });

                let mut hsm_group_from_bos_sessiontemplate_vec =
                    hsm_group_from_bos_sessiontemplate_computer_related_to_cfs_configuration
                        .chain(hsm_group_from_bos_sessiontemplate_uan_related_to_cfs_configuration)
                        .collect::<Vec<String>>();

                hsm_group_from_bos_sessiontemplate_vec.sort();
                hsm_group_from_bos_sessiontemplate_vec.dedup();

                hsm_group_from_bos_sessiontemplate_vec
            }
        }
    }

    pub mod mesa {
        pub mod http_client {
            use std::error::Error;

            use serde_json::Value;

            use crate::hsm::{
                group::shasta::http_client::get_raw,
                r#struct::{HsmGroup, Member},
            };

            pub async fn get(
                shasta_token: &str,
                shasta_base_url: &str,
                shasta_root_cert: &[u8],
                group_name_opt: Option<&String>,
            ) -> Result<Vec<HsmGroup>, reqwest::Error> {
                let response_rslt = get_raw(
                    shasta_token,
                    shasta_base_url,
                    shasta_root_cert,
                    group_name_opt,
                )
                .await;

                let hsm_group_vec: Vec<HsmGroup> = match response_rslt {
                    Ok(response) => {
                        if group_name_opt.is_none() {
                            response.json::<Vec<HsmGroup>>().await.unwrap()
                        } else {
                            vec![response.json::<HsmGroup>().await.unwrap()]
                        }
                    }
                    Err(error) => return Err(error),
                };

                Ok(hsm_group_vec)
            }

            /// https://github.com/Cray-HPE/docs-csm/blob/release/1.5/api/smd.md#post-groups
            pub async fn create_new_hsm_group(
                shasta_token: &str,
                shasta_base_url: &str,
                shasta_root_cert: &[u8],
                hsm_group_name_opt: &str, // label in HSM
                xnames: &[String],
                exclusive: &str,
                description: &str,
                tags: &[String],
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

                // Create the variables that represent our JSON object
                let myxnames = Member {
                    ids: Some(xnames.to_owned()),
                };

                let hsm_group_json = HsmGroup {
                    label: hsm_group_name_opt.to_owned(),
                    description: Option::from(description.to_string().clone()),
                    tags: Option::from(tags.to_owned()),
                    exclusive_group: Option::from(exclusive.to_string().clone()),
                    members: Some(myxnames),
                };
                let hsm_group_json_body = match serde_json::to_string(&hsm_group_json) {
            Ok(m) => m,
            Err(_) => panic!("Error parsing the JSON generated, one or more of the fields could have invalid chars."),
        };

                println!("{:#?}", &hsm_group_json_body);

                let url_api = shasta_base_url.to_owned() + "/smd/hsm/v2/groups";

                let resp = client
                    .post(url_api)
                    .header("Authorization", format!("Bearer {}", shasta_token))
                    .json(&hsm_group_json) // make sure this is not a string!
                    .send()
                    .await?;

                let json_response: Value;

                if resp.status().is_success() {
                    json_response = serde_json::from_str(&resp.text().await?)?;
                } else {
                    println!("Return code: {}\n", resp.status());
                    if resp.status().to_string().to_lowercase().contains("409") {
                        return Err(resp.text().await?.into());
                    } else {
                        return Err(resp.text().await?.into()); // Black magic conversion from Err(Box::new("my error msg")) which does not
                    }
                };

                Ok(json_response.as_array().unwrap().to_owned())
            }

            pub async fn delete_hsm_group(
                shasta_token: &str,
                shasta_base_url: &str,
                shasta_root_cert: &[u8],
                hsm_group_name_opt: &String, // label in HSM
            ) -> Result<String, Box<dyn Error>> {
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
                let url_api =
                    shasta_base_url.to_owned() + "/smd/hsm/v2/groups/" + &hsm_group_name_opt;

                let resp = client
                    .delete(url_api)
                    .header("Authorization", format!("Bearer {}", shasta_token))
                    .send()
                    .await?;

                if resp.status().is_success() {
                    Ok(resp.text().await?)
                } else {
                    log::debug!("delete return code: {}\n", resp.status().to_string());
                    Err(resp.text().await?.into())
                }
            }
        }

        pub mod utils {
            use crate::cfs::session::mesa::r#struct::CfsSessionGetResponse;

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
                cfs_sessions: &[CfsSessionGetResponse],
            ) {
                if let Some(hsm_group_name) = hsm_group {
                    let hsm_group_details =
                        crate::hsm::group::shasta::http_client::get_hsm_group_vec(
                            shasta_token,
                            shasta_base_url,
                            shasta_root_cert,
                            hsm_group,
                        )
                        .await
                        .unwrap();
                    let hsm_group_members =
                        crate::hsm::group::shasta::utils::get_member_vec_from_hsm_group_value_vec(
                            &hsm_group_details,
                        );
                    let cfs_session_hsm_groups: Vec<String> = cfs_sessions
                        .last()
                        .unwrap()
                        .target
                        .as_ref()
                        .unwrap()
                        .groups
                        .as_ref()
                        .unwrap_or(&Vec::new())
                        .iter()
                        .map(|group| group.name.clone())
                        .collect();
                    let cfs_session_members: Vec<String> = cfs_sessions
                        .last()
                        .unwrap()
                        .ansible
                        .as_ref()
                        .unwrap()
                        .limit
                        .clone()
                        .unwrap_or_default()
                        .split(',')
                        .map(|xname| xname.to_string())
                        .collect();
                    if !cfs_session_hsm_groups.contains(hsm_group_name)
                        && !cfs_session_members.iter().all(|cfs_session_member| {
                            hsm_group_members.contains(cfs_session_member)
                        })
                    {
                        println!(
                            "CFS session {} does not apply to HSM group {}",
                            session_name.unwrap(),
                            hsm_group_name
                        );
                        std::process::exit(1);
                    }
                }
            }
        }
    }
}

pub mod component_status {
    pub mod shasta {
        pub mod http_client {

            use reqwest::Url;
            use serde_json::Value;

            pub async fn get_raw(
                shasta_token: &str,
                shasta_base_url: &str,
                shasta_root_cert: &[u8],
                xname_vec: &[String],
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

                let url_params: Vec<_> = xname_vec.iter().map(|xname| ("id", xname)).collect();

                let api_url = Url::parse_with_params(
                    &format!("{}/smd/hsm/v2/State/Components", shasta_base_url),
                    &url_params,
                )
                .unwrap();

                let response_rslt = client
                    .get(api_url.clone())
                    .header("Authorization", format!("Bearer {}", shasta_token))
                    .send()
                    .await;

                match response_rslt {
                    Ok(response) => response.error_for_status(),
                    Err(error) => Err(error),
                }
            }

            /// Fetches nodes/compnents details using HSM v2 ref --> https://apidocs.svc.cscs.ch/iaas/hardware-state-manager/operation/doComponentsGet/
            pub async fn get(
                shasta_token: &str,
                shasta_base_url: &str,
                shasta_root_cert: &[u8],
                xname_vec: &[String],
            ) -> Result<Value, reqwest::Error> {
                let response_rslt =
                    get_raw(shasta_token, shasta_base_url, shasta_root_cert, xname_vec).await;

                let cfs_components_value_vec: Value = match response_rslt {
                    Ok(response) => response.json::<Value>().await.unwrap(),
                    Err(error) => return Err(error),
                };

                Ok(cfs_components_value_vec)
            }
        }
    }
}

pub mod hw_inventory {
    pub mod shasta {
        pub mod http_client {
            use std::error::Error;

            use serde_json::Value;
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
                    let response = serde_json::from_str(&resp.text().await?);
                    log::debug!("response: {:?}", response);
                    Ok(response?)
                } else {
                    Err(resp.json::<Value>().await?["detail"]
                        .as_str()
                        .unwrap()
                        .into()) // Black magic conversion from Err(Box::new("my error msg")) which does not
                }
            }
        }

        pub mod utils {
            use serde_json::Value;

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
                                    .unwrap_or(&serde_json::json!(0))
                                    .as_u64()
                                    .unwrap()
                            })
                            .collect::<Vec<u64>>()
                    })
            }
        }
    }
}
