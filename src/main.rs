#![warn(clippy::pedantic)]

use anyhow::anyhow;
use camino::Utf8PathBuf;
use glob::glob;
use nuget::NuGet;
use pico_args::Arguments;
use quick_xml::{events::Event, Reader};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
    str,
    sync::Arc,
};
use tokio::{process::Command, sync::Semaphore, task::JoinHandle};
use url::Url;

mod nuget;

#[derive(Deserialize)]
struct Package {
    metadata: Metadata,
}

#[derive(Deserialize)]
struct Metadata {
    id: String,
    version: String,
    #[serde(skip)]
    path: PathBuf,
}

struct Res {
    pkg: Metadata,
    url: Url,
    hash: String,
}

async fn get_repos(config_path: &Path) -> anyhow::Result<Vec<Arc<NuGet>>> {
    let mut reader = Reader::from_file(config_path)?;

    let mut buf = Vec::new();
    let mut handles: Vec<JoinHandle<anyhow::Result<_>>> = Vec::new();

    loop {
        match reader.read_event(&mut buf)? {
            Event::Empty(s) | Event::Start(s) if s.name() == b"add" => {
                let attr = s
                    .attributes()
                    .find(|a| a.as_ref().unwrap().key == b"value")
                    .unwrap()?;

                let url = String::from_utf8(attr.unescaped_value()?.into_owned())?;

                if let Ok(url) = Url::parse(&url) {
                    handles.push(tokio::spawn(
                        async move { Ok(Arc::new(NuGet::new(url).await?)) },
                    ));
                }
            }
            Event::End(s) if s.name() == b"packageSources" => break,
            Event::Eof => break,
            _ => {}
        }

        buf.clear();
    }

    let mut repos = Vec::with_capacity(handles.len());

    for fut in handles {
        repos.push(fut.await??);
    }

    Ok(repos)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = Arguments::from_env();

    let dir: Utf8PathBuf = args.value_from_str("--directory")?;
    let nuget_config = args.value_from_str::<_, Utf8PathBuf>("--nuget-config");

    let mut repos = Vec::new();

    if let Ok(nuget_config) = nuget_config {
        repos = get_repos(nuget_config.as_std_path()).await?;
    }

    if repos.is_empty() {
        repos.push(Arc::new(NuGet::nuget_org().await?));
    }

    let mut packages = Vec::new();

    for mut path in glob(dir.join("**/*.nuspec").as_str())?.map(Result::unwrap) {
        let mut pkg: Package = quick_xml::de::from_str(&fs::read_to_string(&path)?)?;

        assert!(path.pop());

        pkg.metadata.path = glob(path.join("*.nupkg").to_str().unwrap())?
            .next()
            .unwrap()?;

        packages.push(pkg.metadata);
    }

    let mut handles = Vec::with_capacity(packages.len());

    let semaphore = Arc::new(Semaphore::new(10)); // TODO: make configurable

    for pkg in packages {
        let repos = repos.clone();
        let semaphore = semaphore.clone();

        handles.push(tokio::spawn(async move {
            let mut url = None;

            for repo in repos {
                if repo.exists(&pkg.id, &pkg.version).await {
                    url = Some(repo.url(&pkg.id, &pkg.version));

                    break;
                }
            }

            if url.is_none() {
                return Err(anyhow!(
                    "couldn't find repo with {} v{}",
                    pkg.id,
                    pkg.version
                ));
            }

            let url = url.unwrap()?;

            let _permit = semaphore.acquire().await?;

            let hash = str::from_utf8(
                &Command::new("nix-hash")
                    .args(&[
                        "--type",
                        "sha256",
                        "--flat",
                        "--base32",
                        pkg.path.to_str().unwrap(),
                    ])
                    .output()
                    .await?
                    .stdout,
            )?
            .trim()
            .to_string();

            Ok(Res { pkg, url, hash })
        }));
    }

    println!("{{fetchNuGet}}: [");

    for fut in handles {
        let res = fut.await??;

        println!(
            "  (fetchNuGet {{ pname = \"{}\"; version = \"{}\"; url = \"{}\"; sha256 = \"{}\"; }})",
            res.pkg.id, res.pkg.version, res.url, res.hash
        );
    }

    println!("]");

    Ok(())
}
