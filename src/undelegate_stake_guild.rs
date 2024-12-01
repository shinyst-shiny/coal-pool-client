use crate::balance::get_token_balance;
use crate::delegate_stake_guild::PoolGuild;
use base64::{prelude::BASE64_STANDARD, Engine};
use clap::Parser;
use colored::*;
use inquire::{InquireError, Text};
use serde::{Deserialize, Serialize};
use solana_sdk::instruction::Instruction;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};
use std::str::FromStr;

#[derive(Debug, Parser)]
pub struct UnStakeFromGuildArgs {
    #[arg(long, value_name = "AMOUNT", help = "Amount of LP to remove.")]
    pub amount: f64,

    #[arg(long, value_name = "MINT", help = "Mint of LP.")]
    pub mint: String,

    #[arg(long, value_name = "MEMBER", help = "Address of the guild member.")]
    pub member_address: Pubkey,
}

pub async fn un_stake_from_guild(
    args: UnStakeFromGuildArgs,
    key: Keypair,
    url: String,
    unsecure: bool,
) {
    let base_url = url;
    let client = reqwest::Client::new();
    let url_prefix = if unsecure {
        "http".to_string()
    } else {
        "https".to_string()
    };
    let balance = get_token_balance(
        &args.member_address,
        base_url.clone(),
        unsecure,
        args.mint.clone(),
    )
    .await;

    // Ensure stake amount does not exceed balance
    let guild_un_stake_amount = if args.amount > balance {
        println!(
            "  You do not have enough LP tokens to un stake {} from the guild.\n  Adjusting un stake amount to the maximum available: {} LP tokens",
            args.amount, balance
        );
        balance
    } else {
        args.amount
    };

    // RED TEXT
    match Text::new(
        &format!(
            "  Are you sure you want to un stake {} LP tokens tokens? (Y/n or 'esc' to cancel)",
            guild_un_stake_amount
        )
        .red()
        .to_string(),
    )
    .prompt()
    {
        Ok(confirm) => {
            if confirm.trim().eq_ignore_ascii_case("esc") {
                println!("  Guild un staking canceled.");
                return;
            } else if confirm.trim().is_empty() || confirm.trim().to_lowercase() == "y" {
                // Proceed with staking
            } else {
                println!("  Guild un staking canceled.");
                return;
            }
        }
        Err(InquireError::OperationCanceled) => {
            println!("  Guild un staking operation canceled.");
            return;
        }
        Err(_) => {
            println!("  Invalid input. Guild un staking canceled.");
            return;
        }
    }

    let resp = client
        .get(format!(
            "{}://{}/pool/fee_payer/pubkey",
            url_prefix, base_url
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let fee_pubkey = Pubkey::from_str(&resp).unwrap();

    // check if the pubkey is of a member of the guild
    if let Ok(err) = client
        .get(format!(
            "{}://{}/guild/check-member?pubkey={}",
            url_prefix,
            base_url,
            key.pubkey().to_string()
        ))
        .send()
        .await
    {
        match err.status() {
            reqwest::StatusCode::NOT_FOUND => {
                println!("  The public key is not initialized for guilds yet, cancelling process");
                return;
            }
            reqwest::StatusCode::OK => {
                println!("  The public key is not in any guild, cancelling the process");
                return;
            }
            reqwest::StatusCode::FOUND => {
                println!("  The public key is is in the guild");
            }
            _ => {
                println!(
                    "  Impossible to check the user guild. Error: {}",
                    err.text().await.unwrap()
                );
                return;
            }
        }
    }

    let resp = client
        .get(format!("{}://{}/guild/addresses", url_prefix, base_url))
        .send()
        .await
        .unwrap();
    let guild: PoolGuild = resp.json().await.unwrap();
    let guild_pubkey = Pubkey::from_str(&guild.pubkey).unwrap();

    let guild_un_stake_amount_u64 =
        (guild_un_stake_amount * 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64)) as u64;

    // we have all the basic info, let's start building the transaction
    let mut ixs: Vec<Instruction> = vec![];

    // now we add the actual stake instruction
    ixs.extend([coal_guilds_api::sdk::unstake(
        key.pubkey(),
        guild_pubkey,
        guild_un_stake_amount_u64,
    )]);

    let resp = client
        .get(format!("{}://{}/latest-blockhash", url_prefix, base_url))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let decoded_blockhash = BASE64_STANDARD.decode(resp).unwrap();
    let deserialized_blockhash = bincode::deserialize(&decoded_blockhash).unwrap();

    let mut tx = Transaction::new_with_payer(&ixs, Some(&fee_pubkey));

    tx.partial_sign(&[&key], deserialized_blockhash);
    let serialized_tx = bincode::serialize(&tx).unwrap();
    let encoded_tx = BASE64_STANDARD.encode(&serialized_tx);

    let resp = client
        .post(format!(
            "{}://{}/guild/unstake?pubkey={}&mint={}&amount={}",
            url_prefix,
            base_url,
            key.pubkey().to_string(),
            args.mint,
            guild_un_stake_amount_u64
        ))
        .body(encoded_tx)
        .send()
        .await;
    if let Ok(res) = resp {
        if let Ok(txt) = res.text().await {
            match txt.as_str() {
                "SUCCESS" => {
                    println!("  Successfully removed staked from guild!");
                }
                other => {
                    println!("  Transaction failed: {}", other);
                }
            }
        } else {
            println!("  Transaction failed, please wait and try again.");
        }
    } else {
        println!("  Transaction failed, please wait and try again.");
    }
}
