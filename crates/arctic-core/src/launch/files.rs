//! Resolving and downloading everything a version needs: client jar,
//! libraries, natives, assets and the logging config.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::IoContext;
use crate::net::{DownloadJob, download_all};
use crate::storage::{DataDirs, load_json};
use crate::versions::model::{Artifact, AssetIndex};
use crate::versions::{Library, RuleEnv, VersionJson, maven_path, rules_allow};
use crate::{Error, Result};

const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

/// A legacy natives jar to unpack into the natives directory.
#[derive(Debug, Clone)]
pub struct NativeJar {
    pub path: PathBuf,
    pub exclude: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Libraries {
    pub classpath: Vec<PathBuf>,
    pub natives: Vec<NativeJar>,
    pub jobs: Vec<DownloadJob>,
}

/// Work out which library files apply on this platform.
pub fn resolve_libraries(dirs: &DataDirs, version: &VersionJson, env: &RuleEnv) -> Libraries {
    let mut out = Libraries::default();
    let mut seen = HashSet::new();
    for lib in version
        .libraries
        .iter()
        .filter(|l| rules_allow(l.rules.as_deref(), env))
    {
        if let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref())
            && let Some(job) = library_job(dirs, lib, artifact, None)
            && seen.insert(job.dest.clone())
        {
            out.classpath.push(job.dest.clone());
            out.jobs.push(job);
        }
        if let Some(classifier) = native_classifier(lib, env)
            && let Some(artifact) = lib
                .downloads
                .as_ref()
                .and_then(|d| d.classifiers.as_ref())
                .and_then(|c| c.get(&classifier))
            && let Some(job) = library_job(dirs, lib, artifact, Some(&classifier))
            && seen.insert(job.dest.clone())
        {
            out.natives.push(NativeJar {
                path: job.dest.clone(),
                exclude: lib.extract.clone().unwrap_or_default().exclude,
            });
            out.jobs.push(job);
        }
    }
    out
}

/// Legacy natives classifier for this OS, e.g. `natives-windows-64`.
fn native_classifier(lib: &Library, env: &RuleEnv) -> Option<String> {
    let bits = if env.arch == "x86" { "32" } else { "64" };
    lib.natives
        .as_ref()?
        .get(env.os)
        .map(|c| c.replace("${arch}", bits))
}

fn library_job(
    dirs: &DataDirs,
    lib: &Library,
    artifact: &Artifact,
    classifier: Option<&str>,
) -> Option<DownloadJob> {
    let rel = artifact.path.clone().or_else(|| {
        let name = match classifier {
            Some(c) => format!("{}:{c}", lib.name),
            None => lib.name.clone(),
        };
        maven_path(&name)
    })?;
    Some(DownloadJob {
        url: artifact.url.clone(),
        dest: dirs.libraries().join(rel),
        sha1: artifact.sha1.clone(),
        size: artifact.size,
        lzma: None,
    })
}

/// Client jar path + download job.
pub fn client_job(dirs: &DataDirs, version: &VersionJson) -> Result<DownloadJob> {
    let client = version
        .downloads
        .as_ref()
        .map(|d| &d.client)
        .ok_or_else(|| Error::Other(format!("version {} has no client download", version.id)))?;
    Ok(DownloadJob {
        url: client.url.clone(),
        dest: dirs
            .version_dir(&version.id)
            .join(format!("{}.jar", version.id)),
        sha1: Some(client.sha1.clone()),
        size: Some(client.size),
        lzma: None,
    })
}

/// Logging config job and the JVM argument template that points at it.
pub fn logging_job(dirs: &DataDirs, version: &VersionJson) -> Option<(DownloadJob, String)> {
    let client = version.logging.as_ref()?.client.as_ref()?;
    let job = DownloadJob {
        url: client.file.url.clone(),
        dest: dirs.assets().join("log_configs").join(&client.file.id),
        sha1: Some(client.file.sha1.clone()),
        size: Some(client.file.size),
        lzma: None,
    };
    Some((job, client.argument.clone()))
}

/// Assets still to fetch plus how the game should find them.
pub struct AssetPlan {
    pub jobs: Vec<DownloadJob>,
    pub index_name: String,
    /// Value for `${game_assets}` (legacy versions read loose files from here).
    pub game_assets: PathBuf,
    index: AssetIndex,
    objects: PathBuf,
}

impl AssetPlan {
    /// After `jobs` are downloaded: lay out the loose copies legacy
    /// (pre-1.7) versions expect.
    pub fn finish(&self) -> Result<()> {
        if self.index.map_to_resources || self.index.is_virtual {
            copy_legacy_assets(&self.index, &self.objects, &self.game_assets)?;
        }
        Ok(())
    }
}

/// Fetch the asset index (small, verified) and list every object.
pub fn plan_assets(dirs: &DataDirs, version: &VersionJson, game_dir: &Path) -> Result<AssetPlan> {
    let index_ref = version
        .asset_index
        .as_ref()
        .ok_or_else(|| Error::Other(format!("version {} has no asset index", version.id)))?;
    let index_path = dirs
        .assets()
        .join("indexes")
        .join(format!("{}.json", index_ref.id));
    let index_job = DownloadJob {
        url: index_ref.url.clone(),
        dest: index_path.clone(),
        sha1: Some(index_ref.sha1.clone()),
        size: Some(index_ref.size),
        lzma: None,
    };
    download_all("Asset index", vec![index_job], &|_| {})?;
    let index: AssetIndex = load_json(&index_path)?
        .ok_or_else(|| Error::Other("asset index missing after download".into()))?;

    let objects = dirs.assets().join("objects");
    let jobs = index
        .objects
        .values()
        .map(|o| {
            let prefix = &o.hash[..2.min(o.hash.len())];
            DownloadJob {
                url: format!("{RESOURCES_URL}/{prefix}/{}", o.hash),
                dest: objects.join(prefix).join(&o.hash),
                sha1: Some(o.hash.clone()),
                size: Some(o.size),
                lzma: None,
            }
        })
        .collect();
    let game_assets = if index.map_to_resources {
        game_dir.join("resources")
    } else if index.is_virtual {
        dirs.assets().join("virtual").join(&index_ref.id)
    } else {
        dirs.assets()
    };
    Ok(AssetPlan {
        jobs,
        index_name: index_ref.id.clone(),
        game_assets,
        index,
        objects,
    })
}

fn copy_legacy_assets(index: &AssetIndex, objects: &Path, target: &Path) -> Result<()> {
    for (name, obj) in &index.objects {
        let dest = target.join(name);
        if fs::metadata(&dest).is_ok_and(|m| m.len() == obj.size) {
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        let src = objects.join(&obj.hash[..2]).join(&obj.hash);
        fs::copy(&src, &dest).at(&dest)?;
    }
    Ok(())
}

/// Unpack legacy natives jars (LWJGL 2 era) into `natives_dir`.
/// Modern LWJGL 3 extracts its own natives at runtime.
pub fn extract_natives(natives: &[NativeJar], natives_dir: &Path) -> Result<()> {
    fs::create_dir_all(natives_dir).at(natives_dir)?;
    for jar in natives {
        let file = fs::File::open(&jar.path).at(&jar.path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| Error::Other(format!("{}: {e}", jar.path.display())))?;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| Error::Other(format!("{}: {e}", jar.path.display())))?;
            let excluded = jar
                .exclude
                .iter()
                .any(|ex| entry.name().starts_with(ex.as_str()));
            // `enclosed_name` rejects absolute paths and `..` (zip-slip).
            let Some(rel) = entry.enclosed_name() else {
                continue;
            };
            if excluded || entry.is_dir() {
                continue;
            }
            let dest = natives_dir.join(rel);
            if fs::metadata(&dest).is_ok_and(|m| m.len() == entry.size()) {
                continue;
            }
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).at(parent)?;
            }
            let mut out = fs::File::create(&dest).at(&dest)?;
            std::io::copy(&mut entry, &mut out).at(&dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet as Set;
    use std::io::Write;

    fn env() -> RuleEnv {
        RuleEnv {
            os: "windows",
            arch: "x86_64",
            features: Set::new(),
        }
    }

    #[test]
    fn resolves_classpath_natives_and_rules() {
        let version: VersionJson = serde_json::from_str(
            r#"{"id":"1.8.9","mainClass":"M","libraries":[
                {"name":"a:lib:1","downloads":{"artifact":{"path":"a/lib/1/lib-1.jar","sha1":"x","size":1,"url":"u"}}},
                {"name":"a:lib:1","downloads":{"artifact":{"path":"a/lib/1/lib-1.jar","sha1":"x","size":1,"url":"u"}}},
                {"name":"b:mac:1","rules":[{"action":"allow","os":{"name":"osx"}}],
                 "downloads":{"artifact":{"path":"b/mac.jar","url":"u"}}},
                {"name":"c:plat:2","natives":{"windows":"natives-windows-${arch}"},"extract":{"exclude":["META-INF/"]},
                 "downloads":{"classifiers":{"natives-windows-64":{"path":"c/plat-64.jar","url":"u"}}}}
            ]}"#,
        )
        .unwrap();
        let dirs = DataDirs::new("root");
        let libs = resolve_libraries(&dirs, &version, &env());
        assert_eq!(libs.classpath, [dirs.libraries().join("a/lib/1/lib-1.jar")]);
        assert_eq!(libs.natives.len(), 1);
        assert_eq!(libs.natives[0].exclude, ["META-INF/"]);
        assert_eq!(libs.jobs.len(), 2);
    }

    #[test]
    fn extracts_natives_with_excludes() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("n.jar");
        {
            let mut zip = zip::ZipWriter::new(fs::File::create(&jar).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("lwjgl64.dll", opts).unwrap();
            zip.write_all(b"dll").unwrap();
            zip.start_file("META-INF/MANIFEST.MF", opts).unwrap();
            zip.write_all(b"m").unwrap();
            zip.finish().unwrap();
        }
        let out = dir.path().join("natives");
        let natives = [NativeJar {
            path: jar,
            exclude: vec!["META-INF/".into()],
        }];
        extract_natives(&natives, &out).unwrap();
        assert_eq!(fs::read(out.join("lwjgl64.dll")).unwrap(), b"dll");
        assert!(!out.join("META-INF").exists());
    }
}
