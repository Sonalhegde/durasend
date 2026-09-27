use anyhow::{bail, Context, Result};
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

pub struct UpnpMapping {
    pub control_url: String,
    pub service_type: String,
    pub external_port: u16,
    pub external_ip: Option<String>,
}

impl UpnpMapping {
    /// Attempts to open a port on the local router using UPnP IGD
    pub fn try_map_port(port: u16) -> Result<Self> {
        let (location, service_type) = discover_gateway(Duration::from_millis(2000))
            .context("No UPnP-compatible gateway router discovered")?;

        let control_url = resolve_control_url(&location, &service_type)
            .context("Failed to resolve UPnP control URL from router description")?;

        // Determine local IP on the network
        let local_ip = get_local_ip(&location)?;

        // Request port mapping via SOAP
        add_port_mapping(&control_url, &service_type, port, &local_ip)
            .context("Failed to add UPnP port mapping")?;

        // Query external public IP
        let external_ip = get_external_ip(&control_url, &service_type).ok();

        Ok(Self {
            control_url,
            service_type,
            external_port: port,
            external_ip,
        })
    }

    /// Deletes the port mapping from the router
    pub fn unmap(&self) {
        let _ = delete_port_mapping(&self.control_url, &self.service_type, self.external_port);
    }
}

impl Drop for UpnpMapping {
    fn drop(&mut self) {
        self.unmap();
    }
}

fn discover_gateway(timeout: Duration) -> Result<(String, String)> {
    let socket = UdpSocket::bind("0.0.0.0:0")
        .context("Failed to bind local UDP socket for SSDP")?;
    socket.set_read_timeout(Some(timeout))?;
    socket.set_broadcast(true)?;

    let search_msg = "M-SEARCH * HTTP/1.1\r\n\
                      HOST: 239.255.255.250:1900\r\n\
                      MAN: \"ssdp:discover\"\r\n\
                      MX: 2\r\n\
                      ST: urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\n\r\n";

    let destination: SocketAddr = "239.255.255.250:1900".parse().unwrap();
    socket.send_to(search_msg.as_bytes(), destination)?;

    let mut buf = [0u8; 4096];
    let (len, _) = socket.recv_from(&mut buf)?;
    let response = String::from_utf8_lossy(&buf[..len]);

    let mut location = None;
    for line in response.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.starts_with("location:") {
            location = Some(line[9..].trim().to_string());
            break;
        }
    }

    let location = location.ok_or_else(|| anyhow::anyhow!("SSDP response missing LOCATION header"))?;
    let service_type = "urn:schemas-upnp-org:service:WANIPConnection:1".to_string();

    Ok((location, service_type))
}

fn resolve_control_url(location: &str, service_type: &str) -> Result<String> {
    let xml = http_get(location)?;

    // Search for serviceType and controlURL in XML
    let pos = xml.find(service_type).unwrap_or(0);
    let slice = &xml[pos..];

    let start_tag = "<controlURL>";
    let end_tag = "</controlURL>";

    if let Some(start) = slice.find(start_tag) {
        if let Some(end) = slice[start + start_tag.len()..].find(end_tag) {
            let rel_url = &slice[start + start_tag.len()..start + start_tag.len() + end];
            return Ok(combine_urls(location, rel_url.trim()));
        }
    }

    // Default fallback
    Ok(combine_urls(location, "/ctl/IPConn"))
}

fn add_port_mapping(
    control_url: &str,
    service_type: &str,
    port: u16,
    internal_client: &str,
) -> Result<()> {
    let body = format!(
        "<?xml version=\"1.0\"?>\r\n\
         <s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\r\n\
         <s:Body>\r\n\
         <u:AddPortMapping xmlns:u=\"{}\">\r\n\
         <NewRemoteHost></NewRemoteHost>\r\n\
         <NewExternalPort>{}</NewExternalPort>\r\n\
         <NewProtocol>TCP</NewProtocol>\r\n\
         <NewInternalPort>{}</NewInternalPort>\r\n\
         <NewInternalClient>{}</NewInternalClient>\r\n\
         <NewEnabled>1</NewEnabled>\r\n\
         <NewPortMappingDescription>SmartXfer</NewPortMappingDescription>\r\n\
         <NewLeaseDuration>0</NewLeaseDuration>\r\n\
         </u:AddPortMapping>\r\n\
         </s:Body>\r\n\
         </s:Envelope>",
        service_type, port, port, internal_client
    );

    let action = format!("\"{}#AddPortMapping\"", service_type);
    let resp = http_soap_post(control_url, &action, &body)?;

    if resp.contains("Fault") || resp.contains("error") {
        bail!("Router rejected UPnP AddPortMapping: {}", resp);
    }

    Ok(())
}

fn get_external_ip(control_url: &str, service_type: &str) -> Result<String> {
    let body = format!(
        "<?xml version=\"1.0\"?>\r\n\
         <s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\r\n\
         <s:Body>\r\n\
         <u:GetExternalIPAddress xmlns:u=\"{}\"/>\r\n\
         </s:Body>\r\n\
         </s:Envelope>",
        service_type
    );

    let action = format!("\"{}#GetExternalIPAddress\"", service_type);
    let resp = http_soap_post(control_url, &action, &body)?;

    let start_tag = "<NewExternalIPAddress>";
    let end_tag = "</NewExternalIPAddress>";

    if let Some(start) = resp.find(start_tag) {
        if let Some(end) = resp[start + start_tag.len()..].find(end_tag) {
            let ip = &resp[start + start_tag.len()..start + start_tag.len() + end];
            return Ok(ip.trim().to_string());
        }
    }

    bail!("Failed to parse external IP from router response")
}

fn delete_port_mapping(control_url: &str, service_type: &str, port: u16) -> Result<()> {
    let body = format!(
        "<?xml version=\"1.0\"?>\r\n\
         <s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\r\n\
         <s:Body>\r\n\
         <u:DeletePortMapping xmlns:u=\"{}\">\r\n\
         <NewRemoteHost></NewRemoteHost>\r\n\
         <NewExternalPort>{}</NewExternalPort>\r\n\
         <NewProtocol>TCP</NewProtocol>\r\n\
         </u:DeletePortMapping>\r\n\
         </s:Body>\r\n\
         </s:Envelope>",
        service_type, port
    );

    let action = format!("\"{}#DeletePortMapping\"", service_type);
    let _ = http_soap_post(control_url, &action, &body);
    Ok(())
}

fn get_local_ip(location: &str) -> Result<String> {
    let addr = parse_host_port(location)?;
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(addr)?;
    let local = socket.local_addr()?;
    Ok(local.ip().to_string())
}

fn parse_host_port(url: &str) -> Result<String> {
    let no_proto = url.trim_start_matches("http://").trim_start_matches("https://");
    let host_port = no_proto.split('/').next().unwrap_or(no_proto);
    Ok(host_port.to_string())
}

fn combine_urls(base: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    let no_proto = base.trim_start_matches("http://").trim_start_matches("https://");
    let host_port = no_proto.split('/').next().unwrap_or(no_proto);
    let clean_path = path.trim_start_matches('/');
    format!("http://{}/{}", host_port, clean_path)
}

fn http_get(url: &str) -> Result<String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let host_port = parse_host_port(url)?;
    let mut stream = TcpStream::connect(&host_port)?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;

    let path = if let Some(idx) = url.find(&host_port) {
        let rest = &url[idx + host_port.len()..];
        if rest.is_empty() { "/" } else { rest }
    } else {
        "/"
    };

    let req = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, host_port
    );
    stream.write_all(req.as_bytes())?;

    let mut body = String::new();
    stream.read_to_string(&mut body)?;
    Ok(body)
}

fn http_soap_post(url: &str, soap_action: &str, body: &str) -> Result<String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let host_port = parse_host_port(url)?;
    let mut stream = TcpStream::connect(&host_port)?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;

    let path = if let Some(idx) = url.find(&host_port) {
        let rest = &url[idx + host_port.len()..];
        if rest.is_empty() { "/" } else { rest }
    } else {
        "/"
    };

    let req = format!(
        "POST {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Content-Type: text/xml; charset=\"utf-8\"\r\n\
         SOAPAction: {}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n\
         {}",
        path,
        host_port,
        soap_action,
        body.len(),
        body
    );

    stream.write_all(req.as_bytes())?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}
