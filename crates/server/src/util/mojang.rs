//! See [`MojangClient`].

use anyhow::{bail, Context};
use flecs_ecs::macros::Component;
use serde_json::Value;
use uuid::Uuid;

fn username_url(username: &str) -> String {
    // format!("https://api.mojang.com/users/profiles/minecraft/{username}")
    format!("https://mowojang.matdoes.dev/users/profiles/minecraft/{username}")
}

fn uuid_url(uuid: &Uuid) -> String {
    // format!("https://sessionserver.mojang.com/session/minecraft/profile/{uuid}?unsigned=false")
    format!("https://mowojang.matdoes.dev/session/minecraft/profile/{uuid}?unsigned=false")
}

/// A client to interface with the Mojang API.
///
/// This uses [matdoes/mowojang](https://matdoes.dev/minecraft-uuids) as a primary source of data.
/// This does not include caching, this should be done separately probably using [`crate::util::db::Db`].
///
/// todo: add Mojang API backup
#[derive(Component, Debug, Default)]
pub struct MojangClient {
    req: reqwest::Client,
}

// todo: add cache for MojangUtils
impl MojangClient {
    /// Gets a player's UUID from their username.
    pub async fn get_uuid(&self, username: &str) -> anyhow::Result<Uuid> {
        let url = username_url(username);
        let json_object = self.response_raw(&url).await?;
        let id = json_object["id"].as_str().context("UUID not found")?;
        Uuid::parse_str(id).map_err(Into::into)
    }

    /// Gets a player's username from their UUID.
    pub async fn get_username(&self, uuid: Uuid) -> anyhow::Result<String> {
        let url = uuid_url(&uuid);
        let json_object = self.response_raw(&url).await?;
        json_object["name"]
            .as_str()
            .map(String::from)
            .context("Username not found")
    }

    /// Gets player data from their UUID.
    pub async fn data_from_uuid(&self, uuid: &Uuid) -> anyhow::Result<Value> {
        let url = uuid_url(uuid);
        self.response_raw(&url).await
    }

    /// Gets player data from their username.
    pub async fn data_from_username(&self, username: &str) -> anyhow::Result<Value> {
        let url = username_url(username);
        self.response_raw(&url).await
    }

    async fn response_raw(&self, url: &str) -> anyhow::Result<Value> {
        let response = self.req.get(url).send().await?;
        if response.status().is_success() {
            let body = response.text().await?;
            let json_object = serde_json::from_str::<Value>(&body)?;
            if json_object.get("error").is_some() {
                bail!(
                    "Mojang API Error: {}",
                    json_object["error"].as_str().unwrap_or("Unknown error")
                );
            }
            Ok(json_object)
        } else {
            bail!("Failed to retrieve data from Mojang API");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::util::mojang::MojangClient;

    #[tokio::test]
    async fn test_get_uuid() {
        let mojang = MojangClient::default();

        let uuid = mojang.get_uuid("Emerald_Explorer").await.unwrap();
        let expected = uuid::Uuid::from_str("86271406-1188-44a5-8496-7af10c906204").unwrap();
        assert_eq!(uuid, expected);
    }

    #[tokio::test]
    async fn test_get_username() {
        let mojang = MojangClient::default();

        let username = mojang
            .get_username(uuid::Uuid::from_str("86271406-1188-44a5-8496-7af10c906204").unwrap())
            .await
            .unwrap();
        assert_eq!(username, "Emerald_Explorer");
    }

    #[tokio::test]
    async fn test_retrieve_username() {
        let mojang = MojangClient::default();

        let res = mojang
            .data_from_uuid(&uuid::Uuid::from_str("86271406-1188-44a5-8496-7af10c906204").unwrap())
            .await
            .unwrap();

        let pretty = serde_json::to_string_pretty(&res).unwrap();
        println!("{pretty}");
    }
}
