use core::time;
use std::collections::BTreeMap;
use std::{error::Error, str::FromStr};

use futures::TryStreamExt;

use futures::{io::Lines, AsyncBufReadExt};
use hyper::Uri;
use hyper_socks2::SocksConnector;
use k8s_openapi::api::core::v1::{ConfigMap, Container, Pod};
use kube::{
    api::{AttachParams, AttachedProcess},
    client::ConfigExt,
    config::{
        AuthInfo, Cluster, Context, KubeConfigOptions, Kubeconfig, NamedAuthInfo, NamedCluster,
        NamedContext,
    },
    Api,
};

use futures::StreamExt;

use secrecy::SecretString;
use serde_json::Value;
use termion::color;

pub async fn get_k8s_client_programmatically(
    k8s_api_url: &str,
    shasta_k8s_secrets: Value,
) -> Result<kube::Client, Box<dyn Error>> {
    let shasta_cluster = Cluster {
        server: Some(k8s_api_url.to_string()),
        tls_server_name: Some("kube-apiserver".to_string()), // The value "kube-apiserver" has been taken from the
        // Subject: CN value in the Shasta certificate running
        // this command echo | openssl s_client -showcerts -servername 10.252.1.12 -connect 10.252.1.12:6442 2>/dev/null | openssl x509 -inform pem -noout -text
        insecure_skip_tls_verify: Some(true),
        certificate_authority: None,
        certificate_authority_data: Some(String::from(
            shasta_k8s_secrets["certificate-authority-data"]
                .as_str()
                .unwrap(),
        )),
        proxy_url: None,
        extensions: None,
    };

    let shasta_named_cluster = NamedCluster {
        name: String::from("shasta"),
        cluster: Some(shasta_cluster),
    };

    let shasta_auth_info = AuthInfo {
        username: None,
        password: None,
        token: None,
        token_file: None,
        client_certificate: None,
        client_certificate_data: Some(String::from(
            shasta_k8s_secrets["client-certificate-data"]
                .as_str()
                .unwrap(),
        )),
        client_key: None,
        client_key_data: Some(
            SecretString::from_str(shasta_k8s_secrets["client-key-data"].as_str().unwrap())
                .unwrap(),
        ),
        impersonate: None,
        impersonate_groups: None,
        auth_provider: None,
        exec: None,
    };

    let shasta_named_auth_info = NamedAuthInfo {
        name: String::from("kubernetes-admin"),
        auth_info: Some(shasta_auth_info),
    };

    let shasta_context = Context {
        cluster: String::from("shasta"),
        user: String::from("kubernetes-admin"),
        namespace: None,
        extensions: None,
    };

    let shasta_named_context = NamedContext {
        name: String::from("kubernetes-admin@kubernetes"),
        context: Some(shasta_context),
    };

    let kube_config = Kubeconfig {
        preferences: None,
        clusters: vec![shasta_named_cluster],
        auth_infos: vec![shasta_named_auth_info],
        contexts: vec![shasta_named_context],
        current_context: Some(String::from("kubernetes-admin@kubernetes")),
        extensions: None,
        kind: None,
        api_version: None,
    };

    let kube_config_options = KubeConfigOptions {
        context: Some(String::from("kubernetes-admin@kubernetes")),
        cluster: Some(String::from("shasta")),
        user: Some(String::from("kubernetes-admin")),
    };

    let config = kube::Config::from_custom_kubeconfig(kube_config, &kube_config_options).await?;

    // OPTION 1 --> Native TLS - WORKING
    /* let client = if std::env::var("SOCKS5").is_ok() {
        log::debug!("SOCKS5 enabled");
        let connector = {
            let mut http = hyper::client::HttpConnector::new();
            http.enforce_http(false);
            let proxy = hyper_socks2::SocksConnector {
                proxy_addr: std::env::var("SOCKS5").unwrap().parse::<Uri>().unwrap(),
                auth: None,
                connector: http,
            };
            let mut native_tls_builder = native_tls::TlsConnector::builder();
            native_tls_builder.danger_accept_invalid_certs(true);
            native_tls_builder.danger_accept_invalid_hostnames(true);
            native_tls_builder.use_sni(false);

            let tls = tokio_native_tls::TlsConnector::from(config.native_tls_connector()?);
            hyper_tls::HttpsConnector::from((proxy, tls))
        };

        let service = tower::ServiceBuilder::new()
            .layer(config.base_uri_layer())
            .option_layer(config.auth_layer()?)
            .service(hyper::Client::builder().build(connector));

        kube::Client::new(service, config.default_namespace)
    } else {
        let https = config.openssl_https_connector()?;
        let service = tower::ServiceBuilder::new()
            .layer(config.base_uri_layer())
            .service(hyper::Client::builder().build(https));
        kube::Client::new(service, config.default_namespace)
    }; */

    // OPTION 2 --> rustls - Not working. Probably failing hyper client
    /* let client = if std::env::var("SOCKS5").is_ok() {
        log::debug!("SOCKS5 enabled");
        // let https = config.rustls_https_connector()?;
        let rustls_config = std::sync::Arc::new(config.rustls_client_config()?);
        println!("rustls_config:\n{:#?}", config.rustls_client_config());
        let mut http_connector = hyper::client::HttpConnector::new();
        http_connector.enforce_http(false);
        let socks_http_connector = SocksConnector {
            proxy_addr: std::env::var("SOCKS5").unwrap().parse::<Uri>().unwrap(), // scheme is required by HttpConnector
            auth: None,
            connector: http_connector.clone(),
        };
        // let socks = socks_http_connector.clone().with_tls()?;
        let https_socks_http_connector = hyper_rustls::HttpsConnector::from((
            socks_http_connector.clone(),
            rustls_config.clone(),
        ));

        println!(
            "https_socks_http_connector:\n{:#?}",
            https_socks_http_connector
        );
        // let https_http_connector = hyper_rustls::HttpsConnector::from((http_connector, rustls_config));
        let service = tower::ServiceBuilder::new()
            .layer(config.base_uri_layer())
            .service(hyper::Client::builder().build(https_socks_http_connector));
        kube::Client::new(service, config.default_namespace)
    } else {
        let https = config.openssl_https_connector()?;
        let service = tower::ServiceBuilder::new()
            .layer(config.base_uri_layer())
            .service(hyper::Client::builder().build(https));
        kube::Client::new(service, config.default_namespace)
    }; */

    let client = if std::env::var("SOCKS5").is_ok() {
        log::debug!("SOCKS5 enabled");
        let mut http_connector = hyper::client::HttpConnector::new();
        http_connector.enforce_http(false);
        let socks_http_connector = SocksConnector {
            proxy_addr: std::env::var("SOCKS5").unwrap().parse::<Uri>().unwrap(), // scheme is required by HttpConnector
            auth: None,
            connector: http_connector.clone(),
        };

        // HttpsConnector following https://github.com/rustls/hyper-rustls/blob/main/examples/client.rs
        // Get CA root cert
        let mut ca_root_cert_pem_decoded: &[u8] = &base64::decode(
            shasta_k8s_secrets["certificate-authority-data"]
                .as_str()
                .unwrap(),
        )?;

        let ca_root_cert = rustls_pemfile::certs(&mut ca_root_cert_pem_decoded)?;

        // Import CA cert into rustls ROOT certificate store
        let mut root_cert_store = tokio_rustls::rustls::RootCertStore::empty();

        root_cert_store.add_parsable_certificates(&ca_root_cert);

        // Prepare client authentication https://github.com/rustls/rustls/blob/0018e7586c2dc689eb9e1ba8e0283c0f24b9fe8c/examples/src/bin/tlsclient-mio.rs#L414-L426
        // Get client cert
        let mut client_cert_pem_decoded: &[u8] = &base64::decode(
            shasta_k8s_secrets["client-certificate-data"]
                .as_str()
                .unwrap(),
        )?;

        let client_certs = rustls_pemfile::certs(&mut client_cert_pem_decoded)
            .unwrap()
            .iter()
            .map(|cert| tokio_rustls::rustls::Certificate(cert.clone()))
            .collect();

        // Get client key
        let mut client_key_decoded: &[u8] =
            &base64::decode(shasta_k8s_secrets["client-key-data"].as_str().unwrap())?;

        let client_key = match rustls_pemfile::read_one(&mut client_key_decoded)
            .expect("cannot parse private key .pem file")
        {
            Some(rustls_pemfile::Item::RSAKey(key)) => tokio_rustls::rustls::PrivateKey(key),
            Some(rustls_pemfile::Item::PKCS8Key(key)) => tokio_rustls::rustls::PrivateKey(key),
            Some(rustls_pemfile::Item::ECKey(key)) => tokio_rustls::rustls::PrivateKey(key),
            _ => tokio_rustls::rustls::PrivateKey(Vec::new()),
        };

        // Create HTTPS connector
        let rustls_config = tokio_rustls::rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_cert_store)
            // .with_no_client_auth();
            .with_client_auth_cert(client_certs, client_key)?;

        let rustls_config = std::sync::Arc::new(rustls_config);

        let args = (socks_http_connector, rustls_config);
        let https_socks_http_connector = hyper_rustls::HttpsConnector::from(args);

        /* let https_socks_http_connector = socks_http_connector
        .with_rustls_root_cert_store(root_cert_store); */

        // Create HTTPS client
        let hyper_client = hyper::Client::builder().build(https_socks_http_connector);

        let service = tower::ServiceBuilder::new()
            .layer(config.base_uri_layer())
            .service(hyper_client);

        kube::Client::new(service, config.default_namespace)
    } else {
        let https = config.openssl_https_connector()?;
        let service = tower::ServiceBuilder::new()
            .layer(config.base_uri_layer())
            .service(hyper::Client::builder().build(https));
        kube::Client::new(service, config.default_namespace)
    };

    Ok(client)
}

pub async fn get_init_container_logs_stream(
    cfs_session_layer_container: &Container,
    cfs_session_pod: &Pod,
    pods_api: &Api<Pod>,
) -> Result<Lines<impl AsyncBufReadExt>, Box<dyn Error + Sync + Send>> {
    log::info!(
        "Looking for container '{}'",
        cfs_session_layer_container.name
    );

    println!(
        "\n{}####{} Init container {}'{}'{} logs\n",
        color::Fg(color::Green),
        color::Fg(color::Reset),
        color::Fg(color::Blue),
        cfs_session_layer_container.name,
        color::Fg(color::Reset),
    );

    let container_log_stream = pods_api
        .log_stream(
            cfs_session_pod.metadata.name.as_ref().unwrap(),
            &kube::api::LogParams {
                follow: true,
                container: Some(cfs_session_layer_container.name.clone()),
                ..kube::api::LogParams::default()
            },
        )
        .await?
        .lines();

    // We are going to use chain method (https://dtantsur.github.io/rust-openstack/tokio/stream/trait.StreamExt.html#method.chain) to join streams coming from kube_client::api::subresource::Api::log_stream which returns Result<impl Stream<Item = Result<Bytes>>> or Result<hyper::body::Bytes>, we will consume the Result hence we will be chaining streams of hyper::body::Bytes
    // container_log_stream = container_log_stream.chain(logs_stream).boxed();

    Ok(container_log_stream)
}

pub async fn get_container_logs_stream(
    cfs_session_layer_container: &Container,
    cfs_session_pod: &Pod,
    pods_api: &Api<Pod>,
) -> Result<Lines<impl AsyncBufReadExt>, Box<dyn Error + Sync + Send>> {
    log::info!(
        "Looking for container '{}'",
        cfs_session_layer_container.name
    );

    println!(
        "\n{}####{} Container {}'{}'{} logs\n",
        color::Fg(color::Green),
        color::Fg(color::Reset),
        color::Fg(color::Blue),
        cfs_session_layer_container.name,
        color::Fg(color::Reset),
    );

    let container_log_stream = pods_api
        .log_stream(
            cfs_session_pod.metadata.name.as_ref().unwrap(),
            &kube::api::LogParams {
                follow: true,
                container: Some(cfs_session_layer_container.name.clone()),
                ..kube::api::LogParams::default()
            },
        )
        .await?
        .lines();

    // We are going to use chain method (https://dtantsur.github.io/rust-openstack/tokio/stream/trait.StreamExt.html#method.chain) to join streams coming from kube_client::api::subresource::Api::log_stream which returns Result<impl Stream<Item = Result<Bytes>>> or Result<hyper::body::Bytes>, we will consume the Result hence we will be chaining streams of hyper::body::Bytes
    // container_log_stream = container_log_stream.chain(logs_stream).boxed();

    Ok(container_log_stream)
}

pub async fn print_cfs_session_logs(client: kube::Client, cfs_session_name: &str) {
    let logs_stream_rslt =
        get_cfs_session_container_git_clone_logs_stream(client.clone(), cfs_session_name).await;

    match logs_stream_rslt {
        Ok(mut logs_stream) => {
            while let Some(line) = logs_stream.try_next().await.unwrap() {
                println!("{}", line);
            }
        }
        Err(error_msg) => log::error!("{}", error_msg),
    }

    let mut logs_stream = get_cfs_session_container_ansible_logs_stream(client, cfs_session_name)
        .await
        .unwrap();

    while let Some(line) = logs_stream.try_next().await.unwrap() {
        println!("{}", line);
    }
}

pub async fn get_configmap(
    client: kube::Client,
    configmap_name: &str,
) -> Option<BTreeMap<String, String>> {
    let configmap_api: kube::Api<ConfigMap> = kube::Api::namespaced(client, "services");

    let params =
        kube::api::ListParams::default().fields(&("metadata.name=".to_owned() + configmap_name));

    let configmap_rslt = configmap_api.list(&params).await;

    if let Err(error) = configmap_rslt {
        println!("{:#?}", error);
        std::process::exit(1);
    }

    let configmap = configmap_rslt.unwrap();

    let configmap_data = &configmap.items.first().unwrap().data;

    configmap_data.clone()
}

pub async fn get_cfs_session_container_git_clone_logs_stream(
    client: kube::Client,
    cfs_session_name: &str,
) -> Result<Lines<impl AsyncBufReadExt>, Box<dyn Error + std::marker::Send + Sync>> {
    let init_container_name = "git-clone";

    let pods_api: kube::Api<Pod> = kube::Api::namespaced(client, "services");

    let params = kube::api::ListParams::default()
        .limit(1)
        .labels(format!("cfsession={}", cfs_session_name).as_str());

    let mut pods = pods_api.list(&params).await?;

    log::debug!(
        "Pods related to CFS session '{}' found are:\n'{:#?}'",
        cfs_session_name,
        pods,
    );

    let mut i = 0;
    let max = 4;
    let delay_secs = 2;

    // Waiting for pod to start
    while pods.items.is_empty() && i <= max {
        println!(
            "Pod for cfs session '{}' not ready. Trying again in {} secs. Attempt {} of {}",
            cfs_session_name,
            delay_secs,
            i + 1,
            max
        );
        i += 1;
        tokio::time::sleep(time::Duration::from_secs(delay_secs)).await;
        pods = pods_api.list(&params).await?;
    }

    if pods.items.is_empty() {
        return Err(format!(
            "Pod for cfs session {}; not ready. Aborting operation",
            cfs_session_name
        )
        .into());
    }

    let params = kube::api::ListParams::default()
        .limit(1)
        .labels(format!("cfsession={}", cfs_session_name).as_str());

    let mut pods = pods_api.list(&params).await?;

    let mut i = 0;
    let max = 30;
    let delay_secs = 2;

    // Waiting for pod to start
    while pods.items.is_empty() && i <= max {
        println!(
            "Pod for cfs session '{}' not ready. Trying again in {} secs. Attempt {} of {}",
            cfs_session_name,
            delay_secs,
            i + 1,
            max
        );
        i += 1;
        tokio::time::sleep(time::Duration::from_secs(delay_secs)).await;
        pods = pods_api.list(&params).await?;
    }

    if pods.items.is_empty() {
        return Err(format!(
            "Pod for cfs session {}; not ready. Aborting operation",
            cfs_session_name
        )
        .into());
    }

    let cfs_session_pod = &pods.items[0].clone();

    let cfs_session_pod_name = cfs_session_pod.metadata.name.clone().unwrap();
    log::info!("Pod name: {}", cfs_session_pod_name);

    let mut init_container_vec = cfs_session_pod
        .spec
        .as_ref()
        .unwrap()
        .init_containers
        .clone()
        .unwrap();

    log::debug!(
        "Init containers found in pod {}: {:#?}",
        cfs_session_pod_name,
        init_container_vec
    );

    let mut git_clone_container: &Container = init_container_vec
        .iter()
        .find(|container| container.name.eq(init_container_name))
        .unwrap();

    log::info!(
        "Fetching logs for contaianer {} in namespace/pod {}/{}",
        init_container_name,
        cfs_session_pod.clone().metadata.namespace.unwrap(),
        cfs_session_pod.clone().metadata.name.unwrap(),
    );

    let init_container_status = cfs_session_pod
        .status
        .clone()
        .unwrap()
        .init_container_statuses
        .unwrap()
        .into_iter()
        .find(|init_container| init_container.name.eq(&git_clone_container.name));

    log::debug!("Container status:\n{:#?}", init_container_status);

    let mut i = 0;
    let max = 30;

    /* // Check init container is terminated
    if init_container_status
            .clone()
            .unwrap()
            .state
            .unwrap()
            .terminated
            .is_some() {

        return Err(format!("Init container {} terminated", init_container_name)
        .into());
    } */

    // Waiting for init container to start
    while (init_container_status.is_none()
        || init_container_status
            .clone()
            .unwrap()
            .state
            .unwrap()
            .waiting
            .is_some()/* || !init_container_status
    .clone()
    .unwrap()
    .state
    .unwrap()
    .terminated
    .is_some() */)
        && i <= max
    {
        log::info!(
            "Container {} state {:?}",
            git_clone_container.name,
            init_container_status.clone().unwrap().state.unwrap()
        );
        println!(
            "Waiting for container '{}' to be ready. Checking again in 2 secs. Attempt {} of {}",
            git_clone_container.name,
            i + 1,
            max
        );

        i += 1;
        tokio::time::sleep(time::Duration::from_secs(2)).await;

        let cfs_session_pod = pods_api.list(&params).await?.items[0].clone();

        init_container_vec = cfs_session_pod
            .spec
            .as_ref()
            .unwrap()
            .init_containers
            .clone()
            .unwrap();

        git_clone_container = init_container_vec
            .iter()
            .find(|container| container.name.eq("git-clone"))
            .unwrap();

        log::debug!(
            "Container status:\n{:#?}",
            init_container_status.as_ref().unwrap()
        );
    }

    if init_container_status.is_none()
        || init_container_status
            .unwrap()
            .state
            .unwrap()
            .waiting
            .is_some()
    {
        return Err(format!(
            "Container '{}' not ready. Aborting operation",
            init_container_name
        )
        .into());
    }

    get_init_container_logs_stream(git_clone_container, cfs_session_pod, &pods_api).await
}

pub async fn get_cfs_session_container_ansible_logs_stream(
    client: kube::Client,
    cfs_session_name: &str,
) -> Result<Lines<impl AsyncBufReadExt>, Box<dyn Error + std::marker::Send + Sync>> {
    let container_name = "ansible";

    let pods_api: kube::Api<Pod> = kube::Api::namespaced(client, "services");

    let params = kube::api::ListParams::default()
        .limit(1)
        .labels(format!("cfsession={}", cfs_session_name).as_str());

    let mut pods = pods_api.list(&params).await?;

    let mut i = 0;
    let max = 30;
    let delay_secs = 2;

    // Waiting for pod to start
    while pods.items.is_empty() && i <= max {
        println!(
            "Pod for cfs session {} not ready. Trying again in {} secs. Attempt {} of {}",
            cfs_session_name,
            delay_secs,
            i + 1,
            max
        );
        i += 1;
        tokio::time::sleep(time::Duration::from_secs(delay_secs)).await;
        pods = pods_api.list(&params).await?;
    }

    if pods.items.is_empty() {
        return Err(format!(
            "Pod for cfs session {} not ready. Aborting operation",
            cfs_session_name
        )
        .into());
    }

    let cfs_session_pod = &pods.items[0].clone();

    let cfs_session_pod_name = cfs_session_pod.metadata.name.clone().unwrap();
    log::info!("Pod name: {}", cfs_session_pod_name);

    let mut containers = cfs_session_pod.spec.as_ref().unwrap().containers.iter();

    log::debug!(
        "Containers found in pod {}: {:#?}",
        cfs_session_pod_name,
        containers
    );

    let ansible_container: &Container = containers
        .find(|container| container.name.eq(container_name))
        .unwrap();

    let mut container_status = get_container_status(cfs_session_pod, &ansible_container.name);

    let mut i = 0;
    let max = 300;

    // Waiting for container ansible-x to start
    while container_status.as_ref().is_none()
        || container_status.as_ref().unwrap().waiting.is_some() && i <= max
    {
        println!(
            "Waiting for container '{}' to be ready. Checking again in 2 secs. Attempt {} of {}",
            ansible_container.name,
            i + 1,
            max
        );
        i += 1;
        tokio::time::sleep(time::Duration::from_secs(2)).await;
        let pods = pods_api.list(&params).await?;
        container_status = get_container_status(&pods.items[0], &ansible_container.name);
        log::debug!(
            "Container status:\n{:#?}",
            container_status.as_ref().unwrap()
        );
    }

    if container_status.as_ref().unwrap().waiting.is_some() {
        return Err(format!(
            "Container '{}' not ready. Aborting operation",
            ansible_container.name
        )
        .into());
    }

    get_container_logs_stream(ansible_container, cfs_session_pod, &pods_api).await
}

fn get_container_status(
    pod: &k8s_openapi::api::core::v1::Pod,
    container_name: &String,
) -> Option<k8s_openapi::api::core::v1::ContainerState> {
    let container_status = pod
        .status
        .as_ref()
        .unwrap()
        .container_statuses
        .as_ref()
        .and_then(|status_vec| {
            status_vec
                .iter()
                .find(|container_status| container_status.name.eq(container_name))
        });

    match container_status {
        Some(container_status_aux) => container_status_aux.state.clone(),
        None => None,
    }
}

pub async fn attach_cfs_session_container_target_k8s_service_name(
    client: kube::Client,
    cfs_session_name: &str,
) -> AttachedProcess {
    let pods_fabric: Api<Pod> = Api::namespaced(client.clone(), "services");

    let params = kube::api::ListParams::default()
        .limit(1)
        .labels(format!("cfsession={}", cfs_session_name).as_str());

    let mut pods = pods_fabric.list(&params).await.unwrap();

    let mut i = 0;
    let max = 30;

    // Waiting for pod to start
    while pods.items.is_empty() && i <= max {
        println!(
            "Pod for cfs session {} not ready. Trying again in 2 secs. Attempt {} of {}",
            cfs_session_name,
            i + 1,
            max
        );
        i += 1;
        tokio::time::sleep(time::Duration::from_secs(2)).await;
        pods = pods_fabric.list(&params).await.unwrap();
    }

    if pods.items.is_empty() {
        eprintln!(
            "Pod for cfs session {} not ready. Aborting operation",
            cfs_session_name
        );
        std::process::exit(1);
    }

    let console_operator_pod = &pods.items[0].clone();

    let console_operator_pod_name = console_operator_pod.metadata.name.clone().unwrap();

    let attached = pods_fabric
        .exec(
            &console_operator_pod_name,
            vec![
                "sh",
                "-c",
                "cat /inventory/hosts/01-cfs-generated.yaml | grep cray-ims- | head -n 1",
            ],
            &AttachParams::default()
                .container("cray-console-operator")
                .stderr(false),
        )
        .await
        .unwrap();

    let mut output = get_output(attached).await;
    log::info!("{output}");

    output = output.trim().to_string();

    println!("{output}");

    output
        .strip_prefix("ansible_host: ")
        .unwrap()
        .strip_suffix("-service.ims.svc.cluster.local")
        .unwrap();

    println!("{output}");

    let ansible_target_container_label = output + "-customize";

    println!("{ansible_target_container_label}");

    // Find ansible target container

    let pods_fabric: Api<Pod> = Api::namespaced(client, "ims");

    let params = kube::api::ListParams::default()
        .limit(1)
        .labels(format!("job-name={}", ansible_target_container_label).as_str());

    let mut pods = pods_fabric.list(&params).await.unwrap();

    let mut i = 0;
    let max = 30;

    // Waiting for pod to start
    while pods.items.is_empty() && i <= max {
        println!(
            "Pod for cfs session {} not ready. Trying again in 2 secs. Attempt {} of {}",
            cfs_session_name,
            i + 1,
            max
        );
        i += 1;
        tokio::time::sleep(time::Duration::from_secs(2)).await;
        pods = pods_fabric.list(&params).await.unwrap();
    }

    if pods.items.is_empty() {
        eprintln!(
            "Pod for cfs session {} not ready. Aborting operation",
            cfs_session_name
        );
        std::process::exit(1);
    }

    let console_operator_pod = &pods.items[0].clone();

    log::info!("Connecting to console ansible target container");

    let console_operator_pod_name = console_operator_pod.metadata.name.clone().unwrap();

    let command = vec!["bash"]; // Enter the container and open conman to access node's console
                                // let command = vec!["bash"]; // Enter the container and open bash to start an interactive
                                // terminal session

    let attachment_rslt = pods_fabric
        .exec(
            &console_operator_pod_name,
            command,
            &AttachParams::default()
                .container("sshd")
                .stdin(true)
                .stdout(true)
                .stderr(false) // Note to self: tty and stderr cannot both be true
                .tty(true),
        )
        .await;

    if let Ok(attachment) = attachment_rslt {
        attachment
    } else {
        eprintln!(
            "Error attaching to container 'sshd' in pod {}. Exit",
            console_operator_pod_name
        );
        std::process::exit(1);
    }
}

pub async fn get_output(mut attached: AttachedProcess) -> String {
    let stdout = tokio_util::io::ReaderStream::new(attached.stdout().unwrap());
    let out = stdout
        .filter_map(|r| async { r.ok().and_then(|v| String::from_utf8(v.to_vec()).ok()) })
        .collect::<Vec<_>>()
        .await
        .join("");
    attached.join().await.unwrap();
    out
}
