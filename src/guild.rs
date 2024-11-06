pub async fn join_guild(url: String, unsecure: bool) {
    let guild_address = get_guild_address(url, unsecure).await;

    // TODO explain to the user how to join the guild using coal-cli
}

pub async fn stake_to_guild(url: String, unsecure: bool) {
    let guild_address = get_guild_address(url, unsecure).await;

    // TODO explain to the user how to join stake to the guild using coal-cli
}

async fn get_guild_address(url: String, unsecure: bool) -> String {
    let client = reqwest::Client::new();
    let url_prefix = if unsecure { "http" } else { "https" };

    client
        .get(format!(
            "{}://{}/guild/address",
            url_prefix,
            url
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
}