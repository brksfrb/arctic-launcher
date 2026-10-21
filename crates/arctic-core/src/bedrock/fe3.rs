//! Windows Update's delivery service (FE3), which hands out the download link of an older Bedrock
//! build (the UWP `.appx` packages from before 1.21.120) from its Windows Update id. The request is
//! the one the Microsoft Store itself sends; Release and Preview builds answer without an account.

// Looking a link up only happens when installing, which is Windows only.
#![cfg_attr(not(windows), allow(dead_code))]

use crate::{Error, Result};

const SECURED_URL: &str =
    "https://fe3.delivery.mp.microsoft.com/ClientWebService/client.asmx/secured";
const USER_AGENT: &str = "Windows-Update-Agent/10.0.10011.16384 Client-Protocol/1.81";
/// The device the request claims to come from (entity-escaped, as it sits inside the XML).
const DEVICE_ATTRIBUTES: &str = "E:BranchReadinessLevel=CBB&amp;OSArchitecture=AMD64&amp;App=WU&amp;InstallationType=Client&amp;AppVer=10.0.17134.471&amp;OSVersion=10.0.17134.472&amp;DeviceFamily=Windows.Desktop";
/// Hosts Microsoft's delivery network answers from.
const DELIVERY_HOST: &str = ".delivery.mp.microsoft.com";
/// The answer is a few KB; anything far bigger is not it.
const MAX_ANSWER_BYTES: u64 = 4 * 1024 * 1024;

/// Is `id` a Windows Update id (a GUID)? The ids come from someone else's list and go into XML.
pub(super) fn is_update_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

fn request(update_id: &str) -> String {
    format!(
        r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:a="http://www.w3.org/2005/08/addressing"><s:Header><a:Action s:mustUnderstand="1">http://www.microsoft.com/SoftwareDistribution/Server/ClientWebService/GetExtendedUpdateInfo2</a:Action><a:MessageID>urn:uuid:a68d4c75-ab85-4ca8-87db-136d281a2e28</a:MessageID><a:To s:mustUnderstand="1">{SECURED_URL}</a:To><o:Security s:mustUnderstand="1" xmlns:o="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd"><Timestamp xmlns="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd"><Created>2019-01-01T00:00:00.000Z</Created><Expires>2100-01-01T00:00:00.000Z</Expires></Timestamp><wuws:WindowsUpdateTicketsToken wsu:id="ClientMSA" xmlns:wsu="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd" xmlns:wuws="http://schemas.microsoft.com/msus/2014/10/WindowsUpdateAuthorization"><TicketType Name="AAD" Version="1.0" Policy="MBI_SSL"/></wuws:WindowsUpdateTicketsToken></o:Security></s:Header><s:Body><GetExtendedUpdateInfo2 xmlns="http://www.microsoft.com/SoftwareDistribution/Server/ClientWebService"><updateIDs><UpdateIdentity><UpdateID>{update_id}</UpdateID><RevisionNumber>1</RevisionNumber></UpdateIdentity></updateIDs><infoTypes><XmlUpdateFragmentType>FileUrl</XmlUpdateFragmentType></infoTypes><deviceAttributes>{DEVICE_ATTRIBUTES}</deviceAttributes></GetExtendedUpdateInfo2></s:Body></s:Envelope>"#
    )
}

/// A fresh download link for the package of `update_id`.
pub(super) fn resolve(update_id: &str) -> Result<String> {
    if !is_update_id(update_id) {
        return Err(Error::Other("this version has no valid update id".into()));
    }
    let mut resp = crate::net::agent()
        .post(SECURED_URL)
        .header("Content-Type", "application/soap+xml; charset=utf-8")
        .header("User-Agent", USER_AGENT)
        .send(request(update_id))?;
    let xml = resp
        .body_mut()
        .with_config()
        .limit(MAX_ANSWER_BYTES)
        .read_to_string()?;
    pick(&xml).ok_or_else(|| {
        Error::Other(
            "Microsoft has no download for this version (it may have been withdrawn)".into(),
        )
    })
}

/// The package link in the answer: the signed edge server first, then any other of Microsoft's.
fn pick(xml: &str) -> Option<String> {
    let urls: Vec<String> = urls(xml)
        .into_iter()
        .filter(|u| is_delivery_url(u))
        .collect();
    urls.iter()
        .find(|u| host(u).starts_with("tlu.dl."))
        .or_else(|| urls.first())
        .cloned()
}

fn host(url: &str) -> String {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or("");
    rest.split('/').next().unwrap_or("").to_ascii_lowercase()
}

fn is_delivery_url(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://"))
        && host(url).ends_with(DELIVERY_HOST)
}

/// The `<Url>…</Url>` values of the answer, with XML escapes undone.
fn urls(xml: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<Url>") {
        let after = &rest[start + 5..];
        let Some(end) = after.find("</Url>") else {
            break;
        };
        found.push(
            after[..end]
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&apos;", "'"),
        );
        rest = &after[end + 6..];
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_guids_are_update_ids() {
        assert!(is_update_id("48f0d81e-0cc1-4e97-ab85-c9d3d24e848f"));
        assert!(!is_update_id("48f0d81e-0cc1-4e97-ab85-c9d3d24e848"));
        assert!(!is_update_id("48f0d81e-0cc1-4e97-ab85-c9d3d24e84<f"));
        assert!(!is_update_id("</UpdateID><x/>"));
    }

    #[test]
    fn the_answer_gives_the_edge_link_and_nothing_foreign() {
        let xml = "<a><Url>http://evil.example/x.appx</Url>\
                   <Url>http://dl.delivery.mp.microsoft.com/files/a?P1=1&amp;P2=2</Url>\
                   <Url>http://tlu.dl.delivery.mp.microsoft.com/files/a?P1=1&amp;P2=2</Url></a>";
        assert_eq!(
            pick(xml).unwrap(),
            "http://tlu.dl.delivery.mp.microsoft.com/files/a?P1=1&P2=2"
        );
        assert!(pick("<a><Url>http://evil.example/x</Url></a>").is_none());
        assert!(pick("<a/>").is_none());
    }
}
