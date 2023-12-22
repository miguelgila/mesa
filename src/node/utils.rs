use comfy_table::{Cell, Table};
use regex::Regex;

use crate::{bss, cfs, hsm};

use super::r#struct::NodeDetails;

pub fn validate_xname_format(xname: &str) -> bool {
    let xname_re = Regex::new(r"^x\d{4}c[0-7]s([0-9]|[1-5][0-9]|6[0-4])b[0-1]n[0-7]$").unwrap();

    xname_re.is_match(xname)
}

/// Validates a list of xnames.
/// Checks xnames strings are valid
/// If hsm_group_name if provided, then checks all xnames belongs to that hsm_group
pub async fn validate_xnames(
    shasta_token: &str,
    shasta_base_url: &str,
    shasta_root_cert: &[u8],
    xnames: &[&str],
    hsm_group_name_opt: Option<&String>,
) -> bool {
    let hsm_group_members: Vec<_> = if let Some(hsm_group_name) = hsm_group_name_opt {
        crate::hsm::http_client::get_hsm_group(
            shasta_token,
            shasta_base_url,
            shasta_root_cert,
            hsm_group_name,
        )
        .await
        .unwrap()["members"]["ids"]
            .as_array()
            .unwrap()
            .to_vec()
            .iter()
            .map(|xname| xname.as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    if xnames.iter().any(|&xname| {
        !validate_xname_format(xname)
            || (!hsm_group_members.is_empty() && !hsm_group_members.contains(&xname.to_string()))
    }) {
        return false;
    }

    /* for xname in xnames {
        if !xname_re.is_match(xname) {
            println!("xname {} not a valid format", xname);
        }

        if !hsm_group_members.contains(&xname.to_string()) {
            println!("xname {} not a member of {:?}", xname, hsm_group_members)
        }
    } */

    true
}

/// Get components data.
/// Currently, CSM will throw an error if many xnames are sent in the request, therefore, this
/// method will paralelize multiple calls, each with a batch of xnames
pub async fn get_node_details(
    shasta_token: &str,
    shasta_base_url: &str,
    shasta_root_cert: &[u8],
    hsm_groups_node_list: Vec<String>,
) -> Vec<NodeDetails> {
    let components_status = cfs::component::mesa::http_client::get(
        shasta_token,
        shasta_base_url,
        shasta_root_cert,
        &hsm_groups_node_list,
    )
    .await
    .unwrap();

    // get boot params
    let nodes_boot_params_list = crate::bss::http_client::get_boot_params(
        shasta_token,
        shasta_base_url,
        shasta_root_cert,
        &hsm_groups_node_list,
    )
    .await
    .unwrap();

    // get all cfs configurations so we can link cfs configuration name with its counterpart in the
    // bos sessiontemplate, we are doing this because bos sessiontemplate does not have
    // creation/update time hence i can't sort by date to loop and find out most recent bos
    // sessiontemplate per node. joining cfs configuration and bos sessiontemplate will help to
    // this
    let mut cfs_configuration_value_vec = crate::cfs::configuration::shasta::http_client::get_all(
        shasta_token,
        shasta_base_url,
        shasta_root_cert,
    )
    .await
    .unwrap();

    // reverse list in order to have most recent cfs configuration lastupdate values at front
    cfs_configuration_value_vec.reverse();

    // println!("bos_sessiontemplate_list:\n{:#?}", bos_sessiontemplate_list);

    // get nodes details (nids) from hsm
    let nodes_hsm_info_resp = hsm::http_client::get_components_status(
        shasta_token,
        shasta_base_url,
        shasta_root_cert,
        hsm_groups_node_list.clone(),
    )
    .await
    .unwrap();

    // match node with bot_sessiontemplate and put them in a list
    let mut node_details_list = Vec::new();

    for node in &hsm_groups_node_list {
        // let mut node_details = Vec::new();

        // find component details
        let component_details = components_status
            .iter()
            .find(|component_status| component_status["id"].as_str().unwrap().eq(node))
            .unwrap();

        let desired_configuration = component_details["desiredConfig"]
            .as_str()
            .unwrap_or_default();
        let configuration_status = component_details["configurationStatus"]
            .as_str()
            .unwrap_or_default();
        let enabled = component_details["enabled"].as_bool().unwrap_or_default();
        let error_count = component_details["errorCount"].as_i64().unwrap_or_default();

        // get power status
        let node_hsm_info = nodes_hsm_info_resp["Components"]
            .as_array()
            .unwrap()
            .iter()
            .find(|&component| component["ID"].as_str().unwrap().eq(node))
            .unwrap();

        let node_power_status = node_hsm_info["State"]
            .as_str()
            .unwrap()
            .to_string()
            .to_uppercase();

        let node_nid = format!(
            "nid{:0>6}",
            node_hsm_info["NID"].as_u64().unwrap().to_string()
        );

        /* node_details.push(node.to_string());
        node_details.push(node_nid);
        node_details.push(node_power_status);
        node_details.push(desired_configuration.to_string());
        node_details.push(configuration_status.to_string());
        node_details.push(enabled.to_string());
        node_details.push(error_count.to_string()); */

        // get node boot params (these are the boot params of the nodes with the image the node
        // boot with). the image in the bos sessiontemplate may be different i don't know why. need
        // to investigate
        let node_boot_params = nodes_boot_params_list.iter().find(|&node_boot_param| {
            node_boot_param["hosts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|host_value| host_value.as_str().unwrap())
                .any(|host| host.eq(node))
        });

        // println!("node_boot_params:\n{:#?}", node_boot_params);

        let kernel_image_path_in_boot_params = node_boot_params.unwrap()["kernel"]
            .as_str()
            .unwrap()
            .to_string()
            .trim_start_matches("s3://boot-images/")
            .trim_end_matches("/kernel")
            .to_string()
            .to_owned();

        // node_details.push(kernel_image_path_in_boot_params);

        // node_details_list.push(node_details.to_owned());

        let node_details = NodeDetails {
            xname: node.to_string(),
            nid: node_nid,
            power_status: node_power_status,
            desired_configuration: desired_configuration.to_owned(),
            configuration_status: configuration_status.to_owned(),
            enabled: enabled.to_string(),
            error_count: error_count.to_string(),
            boot_image_id: kernel_image_path_in_boot_params,
        };

        node_details_list.push(node_details);
    }

    /*     let components_status = shasta::cfs::component::http_client::get_multiple_components(
        shasta_token,
        shasta_base_url,
        Some(&hsm_group_nodes_string),
        None,
    )
    .await
    .unwrap(); */

    // get boot params
    let nodes_boot_params_list = bss::http_client::get_boot_params(
        shasta_token,
        shasta_base_url,
        shasta_root_cert,
        &hsm_groups_node_list,
    )
    .await
    .unwrap();

    // get all cfs configurations so we can link cfs configuration name with its counterpart in the
    // bos sessiontemplate, we are doing this because bos sessiontemplate does not have
    // creation/update time hence i can't sort by date to loop and find out most recent bos
    // sessiontemplate per node. joining cfs configuration and bos sessiontemplate will help to
    // this
    let mut cfs_configuration_value_list = cfs::configuration::shasta::http_client::get_all(
        shasta_token,
        shasta_base_url,
        shasta_root_cert,
    )
    .await
    .unwrap();

    // reverse list in order to have most recent cfs configuration lastupdate values at front
    cfs_configuration_value_list.reverse();

    // println!("bos_sessiontemplate_list:\n{:#?}", bos_sessiontemplate_list);

    // get nodes details (nids) from hsm
    let nodes_hsm_info_resp = hsm::http_client::get_components_status(
        shasta_token,
        shasta_base_url,
        shasta_root_cert,
        hsm_groups_node_list.clone(),
    )
    .await
    .unwrap();

    // match node with bot_sessiontemplate and put them in a list
    let mut node_details_list = Vec::new();

    for node in &hsm_groups_node_list {
        // let mut node_details = Vec::new();

        // find component details
        let component_details = components_status
            .iter()
            .find(|component_status| component_status["id"].as_str().unwrap().eq(node))
            .unwrap();

        let desired_configuration = component_details["desiredConfig"]
            .as_str()
            .unwrap_or_default();
        let configuration_status = component_details["configurationStatus"]
            .as_str()
            .unwrap_or_default();
        let enabled = component_details["enabled"].as_bool().unwrap_or_default();
        let error_count = component_details["errorCount"].as_i64().unwrap_or_default();
        // let tags = component_details["tags"].to_string();

        // get power status
        // node_power_status = get_node_power_status(node, &nodes_power_status_resp);
        let node_hsm_info = nodes_hsm_info_resp["Components"]
            .as_array()
            .unwrap()
            .iter()
            .find(|&component| component["ID"].as_str().unwrap().eq(node))
            .unwrap();

        let node_power_status = node_hsm_info["State"]
            .as_str()
            .unwrap()
            .to_string()
            .to_uppercase();

        let node_nid = format!(
            "nid{:0>6}",
            node_hsm_info["NID"].as_u64().unwrap().to_string()
        );

        /* node_details.push(node.to_string());
        node_details.push(node_nid);
        node_details.push(node_power_status);
        node_details.push(desired_configuration.to_string());
        node_details.push(configuration_status.to_string());
        node_details.push(enabled.to_string());
        node_details.push(error_count.to_string()); */

        // get node boot params (these are the boot params of the nodes with the image the node
        // boot with). the image in the bos sessiontemplate may be different i don't know why. need
        // to investigate
        let node_boot_params = nodes_boot_params_list.iter().find(|&node_boot_param| {
            node_boot_param["hosts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|host_value| host_value.as_str().unwrap())
                .any(|host| host.eq(node))
        });

        // println!("node_boot_params:\n{:#?}", node_boot_params);

        let kernel_image_path_in_boot_params = node_boot_params.unwrap()["kernel"]
            .as_str()
            .unwrap()
            .to_string()
            .trim_start_matches("s3://boot-images/")
            .trim_end_matches("/kernel")
            .to_string()
            .to_owned();

        // node_details.push(kernel_image_path_in_boot_params);

        // node_details_list.push(node_details.to_owned());

        let node_details = NodeDetails {
            xname: node.to_string(),
            nid: node_nid,
            power_status: node_power_status,
            desired_configuration: desired_configuration.to_string(),
            configuration_status: configuration_status.to_string(),
            enabled: enabled.to_string(),
            error_count: error_count.to_string(),
            boot_image_id: kernel_image_path_in_boot_params,
        };

        node_details_list.push(node_details);
    }

    node_details_list
}

pub fn print_table(nodes_status: Vec<NodeDetails>) {
    let mut table = Table::new();

    table.set_header(vec![
        "XNAME",
        "NID",
        "Power Status",
        "Desired Configuration",
        "Configuration Status",
        "Enabled",
        "Error Count",
        // "Tags",
        "Image ID (Boot param)",
    ]);

    for node_status in nodes_status {
        table.add_row(vec![
            Cell::new(node_status.xname),
            Cell::new(node_status.nid),
            Cell::new(node_status.power_status),
            Cell::new(node_status.desired_configuration),
            Cell::new(node_status.configuration_status),
            Cell::new(node_status.enabled),
            Cell::new(node_status.error_count),
            Cell::new(node_status.boot_image_id),
        ]);
    }

    println!("{table}");
}
