use base64::prelude::*;
use clap::{arg, Parser};
use colored::*;
use drillx_2::equix;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::{signature::Keypair, signer::Signer};
use spl_token::amount_to_ui_amount;
use std::env;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    ops::{ControlFlow, Range},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::{mpsc::UnboundedSender, Mutex};
use tokio::time::timeout;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        handshake::client::{generate_key, Request},
        Message,
    },
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::database::{AppDatabase, PoolSubmissionResult};

#[derive(Deserialize, Debug)]
pub struct RewardDetails {
    pub total_balance: f64,
    pub total_rewards: f64,
    pub miner_supplied_difficulty: u32,
    pub miner_earned_rewards: f64,
    pub miner_percentage: f64,
}

#[derive(Deserialize, Debug)]
pub struct CoalDetails {
    pub reward_details: RewardDetails,
    pub top_stake: f64,
    pub stake_multiplier: f64,
    pub guild_total_stake: f64,
    pub guild_multiplier: f64,
    pub tool_multiplier: f64,
}

#[derive(Deserialize, Debug)]
pub struct OreBoost {
    pub top_stake: f64,
    pub total_stake: f64,
    pub stake_multiplier: f64,
    pub mint_address: [u8; 32],
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct OreDetails {
    pub reward_details: RewardDetails,
    pub top_stake: f64,
    pub stake_multiplier: f64,
    pub ore_boosts: Vec<OreBoost>,
}

#[derive(Deserialize, Debug)]
pub struct MinerDetails {
    pub total_chromium: f64,
    pub total_coal: f64,
    pub total_ore: f64,
    pub guild_address: [u8; 32],
    pub miner_address: [u8; 32],
}

#[derive(Deserialize, Debug)]
pub struct ServerMessagePoolSubmissionResult {
    pub difficulty: u32,
    pub challenge: [u8; 32],
    pub best_nonce: u64,
    pub active_miners: u32,
    pub coal_details: CoalDetails,
    pub ore_details: OreDetails,
    pub miner_details: MinerDetails,
}

#[derive(Debug)]
pub enum ServerMessage {
    StartMining([u8; 32], Range<u64>, u64),
    PoolSubmissionResult(ServerMessagePoolSubmissionResult),
}

#[derive(Debug, Clone, Copy)]
pub struct ThreadSubmission {
    nonce: u64,
    difficulty: u32,
    pub d: [u8; 16], // digest
}

#[derive(Debug, Clone, Copy)]
pub enum MessageSubmissionSystem {
    Submission(ThreadSubmission),
    Reset,
    Finish,
}

#[derive(Debug, Parser)]
pub struct MinePublicKeyArgs {
    #[arg(
        long,
        value_name = "pubkey",
        default_value = "",
        help = "The public ky to hook the miner to"
    )]
    pub pubkey: String,
    #[arg(
        long,
        value_name = "threads",
        default_value = "4",
        help = "Number of threads to use while mining"
    )]
    pub threads: u32,
    #[arg(
        long,
        value_name = "BUFFER",
        default_value = "0",
        help = "Buffer time in seconds, to send the submission to the server earlier"
    )]
    pub buffer: u32,
}

#[derive(Debug, Parser)]
pub struct MinePrivateKeyArgs {
    #[arg(
        long,
        value_name = "threads",
        default_value = "4",
        help = "Number of threads to use while mining"
    )]
    pub threads: u32,
    #[arg(
        long,
        value_name = "BUFFER",
        default_value = "0",
        help = "Buffer time in seconds, to send the submission to the server earlier"
    )]
    pub buffer: u32,
}

pub async fn mine_public_key(args: MinePublicKeyArgs, url: String, unsecure: bool) {
    if let Ok(user_pubkey) = Pubkey::from_str(args.pubkey.as_str()) {
        let running = Arc::new(AtomicBool::new(true));

        loop {
            let connection_started = Instant::now();
            if !running.load(Ordering::SeqCst) {
                break;
            }

            let base_url = url.clone();

            let client = reqwest::Client::new();

            let http_prefix = if unsecure {
                "http".to_string()
            } else {
                "https".to_string()
            };

            let timestamp = match client
                .get(format!("{}://{}/timestamp", http_prefix, base_url))
                .send()
                .await
            {
                Ok(res) => {
                    if res.status().as_u16() >= 200 && res.status().as_u16() < 300 {
                        if let Ok(ts) = res.text().await {
                            if let Ok(ts) = ts.parse::<u64>() {
                                ts
                            } else {
                                println!("Server response body for /timestamp failed to parse, contact admin.");
                                tokio::time::sleep(Duration::from_secs(5)).await;
                                continue;
                            }
                        } else {
                            println!(
                                "Server response body for /timestamp is empty, contact admin."
                            );
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                    } else {
                        println!(
                            "Failed to get timestamp from server. StatusCode: {}",
                            res.status()
                        );
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                }
                Err(e) => {
                    println!("Failed to get timestamp from server.\nError: {}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            println!("Server Timestamp: {}", timestamp);

            let ws_url_str = if unsecure {
                format!(
                    "ws://{}/v2/ws-pubkey?pubkey={}&timestamp={}",
                    url,
                    user_pubkey.to_string(),
                    timestamp
                )
            } else {
                format!(
                    "wss://{}/v2/ws-pubkey?pubkey={}&timestamp={}",
                    url,
                    user_pubkey.to_string(),
                    timestamp
                )
            };

            println!("ws_url_str -> {}", ws_url_str);

            let url = url::Url::parse(&ws_url_str).expect("Failed to parse server url");
            let host = url.host_str().expect("Invalid host in server url");
            let threads = args.threads;

            println!("Connecting to server...");
            let request = Request::builder()
                .method("GET")
                .uri(url.to_string())
                .header("Sec-Websocket-Key", generate_key())
                .header("Host", host)
                .header("Upgrade", "websocket")
                .header("Connection", "upgrade")
                .header("Sec-Websocket-Version", "13")
                .body(())
                .unwrap();

            match connect_async(request).await {
                Ok((ws_stream, _)) => {
                    println!(
                        "{}{}{}",
                        "Server: ".dimmed(),
                        format!("Connected to network!").blue(),
                        format!(" [{}ms]", connection_started.elapsed().as_millis()).dimmed(),
                    );
                    println!("Connected to network!");

                    let (sender, mut receiver) = ws_stream.split();
                    let (message_sender, mut message_receiver) =
                        tokio::sync::mpsc::unbounded_channel::<ServerMessage>();

                    let (solution_system_message_sender, solution_system_message_receiver) =
                        tokio::sync::mpsc::unbounded_channel::<MessageSubmissionSystem>();

                    let sender = Arc::new(Mutex::new(sender));
                    let app_socket_sender = sender.clone();
                    tokio::spawn(async move {
                        submission_system(
                            None,
                            solution_system_message_receiver,
                            app_socket_sender,
                            user_pubkey,
                        )
                        .await;
                    });

                    let solution_system_submission_sender =
                        Arc::new(solution_system_message_sender);

                    let msend = message_sender.clone();
                    let system_submission_sender = solution_system_submission_sender.clone();
                    let receiver_thread = tokio::spawn(async move {
                        let mut last_start_mine_instant = Instant::now();
                        loop {
                            match timeout(Duration::from_secs(45), receiver.next()).await {
                                Ok(Some(Ok(message))) => {
                                    match process_message(message, msend.clone()) {
                                        ControlFlow::Break(_) => {
                                            break;
                                        }
                                        ControlFlow::Continue(got_start_mining) => {
                                            if got_start_mining {
                                                last_start_mine_instant = Instant::now();
                                            }
                                        }
                                    }

                                    if last_start_mine_instant.elapsed().as_secs() >= 120 {
                                        eprintln!("Last start mining message was over 2 minutes ago. Closing websocket for reconnection.");
                                        break;
                                    }
                                }
                                Ok(Some(Err(e))) => {
                                    eprintln!("Websocket error: {}", e);
                                    break;
                                }
                                Ok(None) => {
                                    eprintln!("Websocket closed gracefully");
                                    break;
                                }
                                Err(_) => {
                                    eprintln!("Websocket receiver timeout, assuming disconnection");
                                    break;
                                }
                            }
                        }

                        println!("Websocket receiver closed or timed out.");
                        println!("Cleaning up channels...");
                        let _ = system_submission_sender.send(MessageSubmissionSystem::Finish);
                        drop(msend);
                        drop(message_sender);
                    });

                    // send Ready message
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .expect("Time went backwards")
                        .as_secs();

                    let msg = now.to_le_bytes();
                    let mut bin_data: Vec<u8> = Vec::new();
                    bin_data.push(0u8);
                    bin_data.extend_from_slice(&user_pubkey.to_bytes());
                    bin_data.extend_from_slice(&msg);

                    let mut lock = sender.lock().await;
                    let _ = lock.send(Message::Binary(bin_data)).await;
                    drop(lock);

                    let (db_sender, mut db_receiver) =
                        tokio::sync::mpsc::unbounded_channel::<PoolSubmissionResult>();

                    tokio::spawn(async move {
                        let app_db = AppDatabase::new();

                        while let Some(msg) = db_receiver.recv().await {
                            app_db.add_new_pool_submission(msg);
                            let total_earnings_coal = amount_to_ui_amount(
                                app_db.get_todays_earnings_coal(),
                                coal_api::consts::TOKEN_DECIMALS,
                            );
                            let total_earnings_ore = amount_to_ui_amount(
                                app_db.get_todays_earnings_ore(),
                                coal_api::consts::TOKEN_DECIMALS,
                            );
                            println!(
                                "Today's Earnings: {} COAL, {} ORE",
                                total_earnings_coal, total_earnings_ore
                            );
                        }
                    });

                    // receive messages
                    let s_system_submission_sender = solution_system_submission_sender.clone();
                    while let Some(msg) = message_receiver.recv().await {
                        let system_submission_sender = s_system_submission_sender.clone();
                        let db_sender = db_sender.clone();
                        tokio::spawn({
                            let message_sender = sender.clone();
                            let running = running.clone();
                            async move {
                                if !running.load(Ordering::SeqCst) {
                                    return;
                                }

                                match msg {
                                    ServerMessage::StartMining(challenge, nonce_range, cutoff) => {
                                        println!(
                                            "\nNext Challenge: {}",
                                            BASE64_STANDARD.encode(challenge)
                                        );
                                        println!(
                                            "Nonce range: {} - {}",
                                            nonce_range.start, nonce_range.end
                                        );
                                        println!("Cutoff in: {}s", cutoff);

                                        // Adjust the cutoff with the buffer
                                        let mut cutoff = cutoff.saturating_sub(args.buffer as u64);
                                        if cutoff > 60 {
                                            cutoff = 55;
                                        }

                                        // Detect if running on Windows and set symbols accordingly
                                        let pb = if env::consts::OS == "windows" {
                                            ProgressBar::new_spinner().with_style(
                                                ProgressStyle::default_spinner()
                                                    .tick_strings(&["-", "\\", "|", "/"]) // Use simple ASCII symbols
                                                    .template("{spinner:.green} {msg}")
                                                    .expect("Failed to set progress bar template"),
                                            )
                                        } else {
                                            ProgressBar::new_spinner().with_style(
                                                ProgressStyle::default_spinner()
                                                    .tick_strings(&[
                                                        "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧",
                                                        "⠇", "⠏",
                                                    ])
                                                    .template("{spinner:.red} {msg}")
                                                    .expect("Failed to set progress bar template"),
                                            )
                                        };

                                        println!();
                                        pb.set_message("Mining...");
                                        pb.enable_steady_tick(Duration::from_millis(120));

                                        // Original mining code
                                        let stop = Arc::new(AtomicBool::new(false));
                                        let hash_timer = Instant::now();
                                        let core_ids = core_affinity::get_core_ids().unwrap();
                                        let nonces_per_thread = 10_000;
                                        let handles = core_ids
                                        .into_iter()
                                        .map(|i| {
                                            let running = running.clone(); // Capture running in thread
                                            let system_submission_sender = system_submission_sender.clone();
                                            let stop_me = stop.clone();
                                            std::thread::spawn({
                                                let mut memory = equix::SolverMemory::new();
                                                move || {
                                                    if (i.id as u32).ge(&threads) {
                                                        return None;
                                                    }

                                                    let _ = core_affinity::set_for_current(i);

                                                    let first_nonce = nonce_range.start
                                                        + (nonces_per_thread * (i.id as u64));
                                                    let mut nonce = first_nonce;
                                                    let mut best_nonce = nonce;
                                                    let mut best_difficulty = 0;
                                                    let mut best_hash = drillx_2::Hash::default();
                                                    let mut total_hashes: u64 = 0;

                                                    loop {
                                                        // Check if Ctrl+C was pressed
                                                        if !running.load(Ordering::SeqCst) {
                                                            return None;
                                                        }

                                                        if stop_me.load(Ordering::Relaxed) {
                                                            break;
                                                        }

                                                        // Create hash
                                                        for hx in drillx_2::get_hashes_with_memory(
                                                            &mut memory,
                                                            &challenge,
                                                            &nonce.to_le_bytes(),
                                                        ) {
                                                            total_hashes += 1;
                                                            let difficulty = hx.difficulty();
                                                            if difficulty.gt(&7) && difficulty.gt(&best_difficulty) {
                                                                let thread_submission = ThreadSubmission {
                                                                    nonce,
                                                                    difficulty,
                                                                    d: hx.d,
                                                                };
                                                                if let Err(_) = system_submission_sender.send(MessageSubmissionSystem::Submission(thread_submission)) {
                                                                    stop_me.store(true, Ordering::Relaxed);
                                                                }
                                                                best_nonce = nonce;
                                                                best_difficulty = difficulty;
                                                                best_hash = hx;
                                                            }
                                                        }

                                                        // Exit if processed nonce range
                                                        if nonce >= nonce_range.end {
                                                            break;
                                                        }

                                                        if nonce % 100 == 0 {
                                                            if hash_timer.elapsed().as_secs().ge(&cutoff) {
                                                                if best_difficulty.ge(&8) {
                                                                    break;
                                                                }
                                                            }
                                                        }

                                                        // Increment nonce
                                                        nonce += 1;
                                                    }

                                                    // Return the best nonce
                                                    Some((
                                                        best_nonce,
                                                        best_difficulty,
                                                        best_hash,
                                                        total_hashes,
                                                    ))
                                                }
                                            })
                                        })
                                        .collect::<Vec<_>>();

                                        // Join handles and return best nonce
                                        let mut best_difficulty = 0;
                                        let mut total_nonces_checked = 0;
                                        for h in handles {
                                            if let Ok(Some((
                                                _nonce,
                                                difficulty,
                                                _hash,
                                                nonces_checked,
                                            ))) = h.join()
                                            {
                                                total_nonces_checked += nonces_checked;
                                                if difficulty > best_difficulty {
                                                    best_difficulty = difficulty;
                                                }
                                            }
                                        }

                                        let hash_time = hash_timer.elapsed();

                                        // Stop the spinner after mining is done
                                        pb.finish_and_clear();

                                        if stop.load(Ordering::Relaxed) {
                                            return;
                                        }

                                        println!("✔ Mining complete!");
                                        println!("Processed: {}", total_nonces_checked);
                                        println!("Hash time: {:?}", hash_time);
                                        let hash_time_secs = hash_time.as_secs();
                                        if hash_time_secs > 0 {
                                            println!(
                                                "Hashpower: {:?} H/s",
                                                total_nonces_checked.saturating_div(hash_time_secs)
                                            );
                                            println!("Client found diff: {}", best_difficulty);
                                        }

                                        let _ = system_submission_sender
                                            .send(MessageSubmissionSystem::Reset);

                                        //tokio::time::sleep(Duration::from_secs(5 + args.buffer as u64)).await;

                                        // Ready up again
                                        let now = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .expect("Time went backwards")
                                            .as_secs();

                                        let msg = now.to_le_bytes();
                                        let mut bin_data: Vec<u8> = Vec::new();
                                        bin_data.push(0u8);
                                        bin_data.extend_from_slice(&user_pubkey.to_bytes());
                                        bin_data.extend_from_slice(&msg);
                                        {
                                            let mut message_sender = message_sender.lock().await;
                                            if let Err(_) =
                                                message_sender.send(Message::Binary(bin_data)).await
                                            {
                                                let _ = system_submission_sender
                                                    .send(MessageSubmissionSystem::Finish);
                                                println!(
                                                    "Failed to send Ready message. Returning..."
                                                );
                                                return;
                                            }
                                        }
                                    }
                                    ServerMessage::PoolSubmissionResult(data) => {
                                        let pool_earned_coal =
                                            (data.coal_details.reward_details.total_rewards
                                                * 10f64
                                                    .powf(coal_api::consts::TOKEN_DECIMALS as f64))
                                                as u64;
                                        let miner_earned_coal =
                                            (data.coal_details.reward_details.miner_earned_rewards
                                                * 10f64
                                                    .powf(coal_api::consts::TOKEN_DECIMALS as f64))
                                                as u64;
                                        let pool_earned_ore =
                                            (data.ore_details.reward_details.total_rewards
                                                * 10f64
                                                    .powf(coal_api::consts::TOKEN_DECIMALS as f64))
                                                as u64;
                                        let miner_earned_ore =
                                            (data.ore_details.reward_details.miner_earned_rewards
                                                * 10f64
                                                    .powf(coal_api::consts::TOKEN_DECIMALS as f64))
                                                as u64;
                                        let miner_total_coal = data.miner_details.total_coal
                                            / 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64);
                                        let miner_total_ore = data.miner_details.total_ore
                                            / 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64);
                                        let miner_total_chromium = data
                                            .miner_details
                                            .total_chromium
                                            / 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64);

                                        let pool_guild =
                                            Pubkey::from(data.miner_details.guild_address)
                                                .to_string();

                                        let miner_address =
                                            Pubkey::from(data.miner_details.miner_address)
                                                .to_string();

                                        let ps = PoolSubmissionResult::new(
                                            data.difficulty,
                                            pool_earned_coal,
                                            pool_earned_ore,
                                            data.coal_details.reward_details.miner_percentage,
                                            data.ore_details.reward_details.miner_percentage,
                                            data.coal_details
                                                .reward_details
                                                .miner_supplied_difficulty,
                                            miner_earned_coal,
                                            miner_earned_ore,
                                        );
                                        let _ = db_sender.send(ps);

                                        let message = format!(
                                            "\n\
                                        \n----------------------\
                                        \nMINER INFO\
                                        \nMiner Address: {}\
                                        \nMiner Balance: {:.11} COAL / {:.11} ORE / {:.11} CHROMIUM\
                                        \n----------------------\
                                        \nPOOL INFO\
                                        \nChallenge: {}\
                                        \nPool Submitted Difficulty: {}\
                                        \nPool Earned: {:.11} COAL / {:.11} ORE\
                                        \nPool Balance: {:.11} COAL / {:.11} ORE\
                                        \nSystem TOP stake: {:.11} COAL\
                                        \nGuild Address: {}\
                                        \nGuild Stake: {:.11} LP\
                                        \nActive Miners: {}\
                                        \n----------------------\
                                        \nCOAL MULTIPLIERS\
                                        \nStake: {:.2}x\
                                        \nTool Multiplier: {:.2}x\
                                        \nGuild Multiplier: {:.2}x\
                                        \nTotal multiplier for each miner: {:.2}x\
                                        \n----------------------\
                                        \nMINER REWARDS\
                                        \nMiner Submitted Difficulty: {}\
                                        \nMiner Earned: {:.11} COAL\
                                        \n{:.2}% of total pool reward\
                                        \nMiner Earned: {:.11} ORE\
                                        \n{:.2}% of total pool reward\n",
                                            miner_address,
                                            miner_total_coal,
                                            miner_total_ore,
                                            miner_total_chromium,
                                            BASE64_STANDARD.encode(data.challenge),
                                            data.difficulty,
                                            data.coal_details.reward_details.total_rewards,
                                            data.ore_details.reward_details.total_rewards,
                                            data.coal_details.reward_details.total_balance,
                                            data.ore_details.reward_details.total_balance,
                                            data.coal_details.top_stake,
                                            pool_guild,
                                            data.coal_details.guild_total_stake
                                                / 10f64
                                                    .powf(coal_api::consts::TOKEN_DECIMALS as f64),
                                            data.active_miners,
                                            data.coal_details.stake_multiplier,
                                            data.coal_details.tool_multiplier,
                                            data.coal_details.guild_multiplier,
                                            data.coal_details.stake_multiplier
                                                * data.coal_details.tool_multiplier
                                                * data.coal_details.guild_multiplier,
                                            data.coal_details
                                                .reward_details
                                                .miner_supplied_difficulty,
                                            data.coal_details.reward_details.miner_earned_rewards,
                                            data.coal_details.reward_details.miner_percentage,
                                            data.ore_details.reward_details.miner_earned_rewards,
                                            data.ore_details.reward_details.miner_percentage
                                        );
                                        println!("{}", message);
                                    }
                                }
                            }
                        });
                    }

                    // If the websocket message receiver finishes, also finish the solution submission
                    // sender system
                    let _ = receiver_thread.await;
                    let _ = solution_system_submission_sender.send(MessageSubmissionSystem::Finish);
                    println!("Channels cleaned up, reconnecting...\n");
                }
                Err(e) => {
                    println!("Failed to connect: {:?}", e);
                    match e {
                        tokio_tungstenite::tungstenite::Error::Http(e) => {
                            if let Some(body) = e.body() {
                                println!("1 Error: {:?}", String::from_utf8(body.to_vec()));
                            } else {
                                println!("2 Http Error: {:?}", e);
                            }
                        }
                        _ => {
                            println!("3 Error: {:?}", e);
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }
    } else {
        eprintln!("Error getting the public key: {}", args.pubkey);
    }
}

pub async fn mine_private_key(args: MinePrivateKeyArgs, key: Keypair, url: String, unsecure: bool) {
    let running = Arc::new(AtomicBool::new(true));
    let key = Arc::new(key);

    loop {
        let connection_started = Instant::now();
        if !running.load(Ordering::SeqCst) {
            break;
        }

        let base_url = url.clone();
        let mut ws_url_str = if unsecure {
            format!("ws://{}/v2/ws", url)
        } else {
            format!("wss://{}/v2/ws", url)
        };

        let client = reqwest::Client::new();

        let http_prefix = if unsecure {
            "http".to_string()
        } else {
            "https".to_string()
        };

        let timestamp = match client
            .get(format!("{}://{}/timestamp", http_prefix, base_url))
            .send()
            .await
        {
            Ok(res) => {
                if res.status().as_u16() >= 200 && res.status().as_u16() < 300 {
                    if let Ok(ts) = res.text().await {
                        if let Ok(ts) = ts.parse::<u64>() {
                            ts
                        } else {
                            println!("Server response body for /timestamp failed to parse, contact admin.");
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                    } else {
                        println!("Server response body for /timestamp is empty, contact admin.");
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                } else {
                    println!(
                        "Failed to get timestamp from server. StatusCode: {}",
                        res.status()
                    );
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            }
            Err(e) => {
                println!("Failed to get timestamp from server.\nError: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        println!("Server Timestamp: {}", timestamp);

        let ts_msg = timestamp.to_le_bytes();
        let sig = key.sign_message(&ts_msg);

        ws_url_str.push_str(&format!("?timestamp={}", timestamp));
        let url = url::Url::parse(&ws_url_str).expect("Failed to parse server url");
        let host = url.host_str().expect("Invalid host in server url");
        let threads = args.threads;

        let auth = BASE64_STANDARD.encode(format!("{}:{}", key.pubkey(), sig));

        println!("Connecting to server...");
        let request = Request::builder()
            .method("GET")
            .uri(url.to_string())
            .header("Sec-Websocket-Key", generate_key())
            .header("Host", host)
            .header("Upgrade", "websocket")
            .header("Connection", "upgrade")
            .header("Sec-Websocket-Version", "13")
            .header("Authorization", format!("Basic {}", auth))
            .body(())
            .unwrap();

        match connect_async(request).await {
            Ok((ws_stream, _)) => {
                println!(
                    "{}{}{}",
                    "Server: ".dimmed(),
                    format!("Connected to network!").blue(),
                    format!(" [{}ms]", connection_started.elapsed().as_millis()).dimmed(),
                );
                println!("Connected to network!");

                let (sender, mut receiver) = ws_stream.split();
                let (message_sender, mut message_receiver) =
                    tokio::sync::mpsc::unbounded_channel::<ServerMessage>();

                let (solution_system_message_sender, solution_system_message_receiver) =
                    tokio::sync::mpsc::unbounded_channel::<MessageSubmissionSystem>();

                let sender = Arc::new(Mutex::new(sender));
                let app_key = key.clone();
                let app_socket_sender = sender.clone();
                tokio::spawn(async move {
                    submission_system(
                        Some(app_key.clone()),
                        solution_system_message_receiver,
                        app_socket_sender,
                        app_key.clone().pubkey(),
                    )
                    .await;
                });

                let solution_system_submission_sender = Arc::new(solution_system_message_sender);

                let msend = message_sender.clone();
                let system_submission_sender = solution_system_submission_sender.clone();
                let receiver_thread = tokio::spawn(async move {
                    let mut last_start_mine_instant = Instant::now();
                    loop {
                        match timeout(Duration::from_secs(45), receiver.next()).await {
                            Ok(Some(Ok(message))) => {
                                match process_message(message, msend.clone()) {
                                    ControlFlow::Break(_) => {
                                        break;
                                    }
                                    ControlFlow::Continue(got_start_mining) => {
                                        if got_start_mining {
                                            last_start_mine_instant = Instant::now();
                                        }
                                    }
                                }

                                if last_start_mine_instant.elapsed().as_secs() >= 120 {
                                    eprintln!("Last start mining message was over 2 minutes ago. Closing websocket for reconnection.");
                                    break;
                                }
                            }
                            Ok(Some(Err(e))) => {
                                eprintln!("Websocket error: {}", e);
                                break;
                            }
                            Ok(None) => {
                                eprintln!("Websocket closed gracefully");
                                break;
                            }
                            Err(_) => {
                                eprintln!("Websocket receiver timeout, assuming disconnection");
                                break;
                            }
                        }
                    }

                    println!("Websocket receiver closed or timed out.");
                    println!("Cleaning up channels...");
                    let _ = system_submission_sender.send(MessageSubmissionSystem::Finish);
                    drop(msend);
                    drop(message_sender);
                });

                // send Ready message
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards")
                    .as_secs();

                let msg = now.to_le_bytes();
                let sig = key.sign_message(&msg).to_string().as_bytes().to_vec();
                let mut bin_data: Vec<u8> = Vec::new();
                bin_data.push(0u8);
                bin_data.extend_from_slice(&key.pubkey().to_bytes());
                bin_data.extend_from_slice(&msg);
                bin_data.extend(sig);

                let mut lock = sender.lock().await;
                let _ = lock.send(Message::Binary(bin_data)).await;
                drop(lock);

                let (db_sender, mut db_receiver) =
                    tokio::sync::mpsc::unbounded_channel::<PoolSubmissionResult>();

                tokio::spawn(async move {
                    let app_db = AppDatabase::new();

                    while let Some(msg) = db_receiver.recv().await {
                        app_db.add_new_pool_submission(msg);
                        let total_earnings_coal = amount_to_ui_amount(
                            app_db.get_todays_earnings_coal(),
                            coal_api::consts::TOKEN_DECIMALS,
                        );
                        let total_earnings_ore = amount_to_ui_amount(
                            app_db.get_todays_earnings_ore(),
                            coal_api::consts::TOKEN_DECIMALS,
                        );
                        println!(
                            "Today's Earnings: {} COAL, {} ORE",
                            total_earnings_coal, total_earnings_ore
                        );
                    }
                });

                // receive messages
                let s_system_submission_sender = solution_system_submission_sender.clone();
                while let Some(msg) = message_receiver.recv().await {
                    let system_submission_sender = s_system_submission_sender.clone();
                    let db_sender = db_sender.clone();
                    tokio::spawn({
                        let message_sender = sender.clone();
                        let key = key.clone();
                        let running = running.clone();
                        async move {
                            if !running.load(Ordering::SeqCst) {
                                return;
                            }

                            match msg {
                                ServerMessage::StartMining(challenge, nonce_range, cutoff) => {
                                    println!(
                                        "\nNext Challenge: {}",
                                        BASE64_STANDARD.encode(challenge)
                                    );
                                    println!(
                                        "Nonce range: {} - {}",
                                        nonce_range.start, nonce_range.end
                                    );
                                    println!("Cutoff in: {}s", cutoff);

                                    // Adjust the cutoff with the buffer
                                    let mut cutoff = cutoff.saturating_sub(args.buffer as u64);
                                    if cutoff > 60 {
                                        cutoff = 55;
                                    }

                                    // Detect if running on Windows and set symbols accordingly
                                    let pb = if env::consts::OS == "windows" {
                                        ProgressBar::new_spinner().with_style(
                                            ProgressStyle::default_spinner()
                                                .tick_strings(&["-", "\\", "|", "/"]) // Use simple ASCII symbols
                                                .template("{spinner:.green} {msg}")
                                                .expect("Failed to set progress bar template"),
                                        )
                                    } else {
                                        ProgressBar::new_spinner().with_style(
                                            ProgressStyle::default_spinner()
                                                .tick_strings(&[
                                                    "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇",
                                                    "⠏",
                                                ])
                                                .template("{spinner:.red} {msg}")
                                                .expect("Failed to set progress bar template"),
                                        )
                                    };

                                    println!();
                                    pb.set_message("Mining...");
                                    pb.enable_steady_tick(Duration::from_millis(120));

                                    // Original mining code
                                    let stop = Arc::new(AtomicBool::new(false));
                                    let hash_timer = Instant::now();
                                    let core_ids = core_affinity::get_core_ids().unwrap();
                                    let nonces_per_thread = 10_000;
                                    let handles = core_ids
                                        .into_iter()
                                        .map(|i| {
                                            let running = running.clone(); // Capture running in thread
                                            let system_submission_sender = system_submission_sender.clone();
                                            let stop_me = stop.clone();
                                            std::thread::spawn({
                                                let mut memory = equix::SolverMemory::new();
                                                move || {
                                                    if (i.id as u32).ge(&threads) {
                                                        return None;
                                                    }

                                                    let _ = core_affinity::set_for_current(i);

                                                    let first_nonce = nonce_range.start
                                                        + (nonces_per_thread * (i.id as u64));
                                                    let mut nonce = first_nonce;
                                                    let mut best_nonce = nonce;
                                                    let mut best_difficulty = 0;
                                                    let mut best_hash = drillx_2::Hash::default();
                                                    let mut total_hashes: u64 = 0;

                                                    loop {
                                                        // Check if Ctrl+C was pressed
                                                        if !running.load(Ordering::SeqCst) {
                                                            return None;
                                                        }

                                                        if stop_me.load(Ordering::Relaxed) {
                                                            break;
                                                        }

                                                        // Create hash
                                                        for hx in drillx_2::get_hashes_with_memory(
                                                            &mut memory,
                                                            &challenge,
                                                            &nonce.to_le_bytes(),
                                                        ) {
                                                            total_hashes += 1;
                                                            let difficulty = hx.difficulty();
                                                            if difficulty.gt(&7) && difficulty.gt(&best_difficulty) {
                                                                let thread_submission = ThreadSubmission {
                                                                    nonce,
                                                                    difficulty,
                                                                    d: hx.d,
                                                                };
                                                                if let Err(_) = system_submission_sender.send(MessageSubmissionSystem::Submission(thread_submission)) {
                                                                    stop_me.store(true, Ordering::Relaxed);
                                                                }
                                                                best_nonce = nonce;
                                                                best_difficulty = difficulty;
                                                                best_hash = hx;
                                                            }
                                                        }

                                                        // Exit if processed nonce range
                                                        if nonce >= nonce_range.end {
                                                            break;
                                                        }

                                                        if nonce % 100 == 0 {
                                                            if hash_timer.elapsed().as_secs().ge(&cutoff) {
                                                                if best_difficulty.ge(&8) {
                                                                    break;
                                                                }
                                                            }
                                                        }

                                                        // Increment nonce
                                                        nonce += 1;
                                                    }

                                                    // Return the best nonce
                                                    Some((
                                                        best_nonce,
                                                        best_difficulty,
                                                        best_hash,
                                                        total_hashes,
                                                    ))
                                                }
                                            })
                                        })
                                        .collect::<Vec<_>>();

                                    // Join handles and return best nonce
                                    let mut best_difficulty = 0;
                                    let mut total_nonces_checked = 0;
                                    for h in handles {
                                        if let Ok(Some((
                                            _nonce,
                                            difficulty,
                                            _hash,
                                            nonces_checked,
                                        ))) = h.join()
                                        {
                                            total_nonces_checked += nonces_checked;
                                            if difficulty > best_difficulty {
                                                best_difficulty = difficulty;
                                            }
                                        }
                                    }

                                    let hash_time = hash_timer.elapsed();

                                    // Stop the spinner after mining is done
                                    pb.finish_and_clear();

                                    if stop.load(Ordering::Relaxed) {
                                        return;
                                    }

                                    println!("✔ Mining complete!");
                                    println!("Processed: {}", total_nonces_checked);
                                    println!("Hash time: {:?}", hash_time);
                                    let hash_time_secs = hash_time.as_secs();
                                    if hash_time_secs > 0 {
                                        println!(
                                            "Hashpower: {:?} H/s",
                                            total_nonces_checked.saturating_div(hash_time_secs)
                                        );
                                        println!("Client found diff: {}", best_difficulty);
                                    }

                                    let _ = system_submission_sender
                                        .send(MessageSubmissionSystem::Reset);

                                    //tokio::time::sleep(Duration::from_secs(5 + args.buffer as u64)).await;

                                    // Ready up again
                                    let now = SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .expect("Time went backwards")
                                        .as_secs();

                                    let msg = now.to_le_bytes();
                                    let sig =
                                        key.sign_message(&msg).to_string().as_bytes().to_vec();
                                    let mut bin_data: Vec<u8> = Vec::new();
                                    bin_data.push(0u8);
                                    bin_data.extend_from_slice(&key.pubkey().to_bytes());
                                    bin_data.extend_from_slice(&msg);
                                    bin_data.extend(sig);
                                    {
                                        let mut message_sender = message_sender.lock().await;
                                        if let Err(_) =
                                            message_sender.send(Message::Binary(bin_data)).await
                                        {
                                            let _ = system_submission_sender
                                                .send(MessageSubmissionSystem::Finish);
                                            println!("Failed to send Ready message. Returning...");
                                            return;
                                        }
                                    }
                                }
                                ServerMessage::PoolSubmissionResult(data) => {
                                    let pool_earned_coal =
                                        (data.coal_details.reward_details.total_rewards
                                            * 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64))
                                            as u64;
                                    let miner_earned_coal =
                                        (data.coal_details.reward_details.miner_earned_rewards
                                            * 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64))
                                            as u64;
                                    let pool_earned_ore =
                                        (data.ore_details.reward_details.total_rewards
                                            * 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64))
                                            as u64;
                                    let miner_earned_ore =
                                        (data.ore_details.reward_details.miner_earned_rewards
                                            * 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64))
                                            as u64;
                                    let miner_total_coal = data.miner_details.total_coal
                                        / 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64);
                                    let miner_total_ore = data.miner_details.total_ore
                                        / 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64);
                                    let miner_total_chromium = data.miner_details.total_chromium
                                        / 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64);

                                    let pool_guild =
                                        Pubkey::from(data.miner_details.guild_address).to_string();

                                    let miner_address =
                                        Pubkey::from(data.miner_details.miner_address).to_string();

                                    let ps = PoolSubmissionResult::new(
                                        data.difficulty,
                                        pool_earned_coal,
                                        pool_earned_ore,
                                        data.coal_details.reward_details.miner_percentage,
                                        data.ore_details.reward_details.miner_percentage,
                                        data.coal_details.reward_details.miner_supplied_difficulty,
                                        miner_earned_coal,
                                        miner_earned_ore,
                                    );
                                    let _ = db_sender.send(ps);

                                    let message = format!(
                                        "\n\
                                        \n----------------------\
                                        \nMINER INFO\
                                        \nMiner Address: {}\
                                        \nMiner Balance: {:.11} COAL / {:.11} ORE / {:.11} CHROMIUM\
                                        \n----------------------\
                                        \nPOOL INFO\
                                        \nChallenge: {}\
                                        \nPool Submitted Difficulty: {}\
                                        \nPool Earned: {:.11} COAL / {:.11} ORE\
                                        \nPool Balance: {:.11} COAL / {:.11} ORE\
                                        \nSystem TOP stake: {:.11} COAL\
                                        \nGuild Address: {}\
                                        \nGuild Stake: {:.11} LP\
                                        \nActive Miners: {}\
                                        \n----------------------\
                                        \nCOAL MULTIPLIERS\
                                        \nStake: {:.2}x\
                                        \nTool Multiplier: {:.2}x\
                                        \nGuild Multiplier: {:.2}x\
                                        \nTotal multiplier for each miner: {:.2}x\
                                        \n----------------------\
                                        \nMINER REWARDS\
                                        \nMiner Submitted Difficulty: {}\
                                        \nMiner Earned: {:.11} COAL\
                                        \n{:.2}% of total pool reward\
                                        \nMiner Earned: {:.11} ORE\
                                        \n{:.2}% of total pool reward\n",
                                        miner_address,
                                        miner_total_coal,
                                        miner_total_ore,
                                        miner_total_chromium,
                                        BASE64_STANDARD.encode(data.challenge),
                                        data.difficulty,
                                        data.coal_details.reward_details.total_rewards,
                                        data.ore_details.reward_details.total_rewards,
                                        data.coal_details.reward_details.total_balance,
                                        data.ore_details.reward_details.total_balance,
                                        data.coal_details.top_stake,
                                        pool_guild,
                                        data.coal_details.guild_total_stake
                                            / 10f64.powf(coal_api::consts::TOKEN_DECIMALS as f64),
                                        data.active_miners,
                                        data.coal_details.stake_multiplier,
                                        data.coal_details.tool_multiplier,
                                        data.coal_details.guild_multiplier,
                                        data.coal_details.stake_multiplier
                                            * data.coal_details.tool_multiplier
                                            * data.coal_details.guild_multiplier,
                                        data.coal_details.reward_details.miner_supplied_difficulty,
                                        data.coal_details.reward_details.miner_earned_rewards,
                                        data.coal_details.reward_details.miner_percentage,
                                        data.ore_details.reward_details.miner_earned_rewards,
                                        data.ore_details.reward_details.miner_percentage
                                    );
                                    println!("{}", message);
                                }
                            }
                        }
                    });
                }

                // If the websocket message receiver finishes, also finish the solution submission
                // sender system
                let _ = receiver_thread.await;
                let _ = solution_system_submission_sender.send(MessageSubmissionSystem::Finish);
                println!("Channels cleaned up, reconnecting...\n");
            }
            Err(e) => {
                match e {
                    tokio_tungstenite::tungstenite::Error::Http(e) => {
                        if let Some(body) = e.body() {
                            println!("Error: {:?}", String::from_utf8(body.to_vec()));
                        } else {
                            println!("Http Error: {:?}", e);
                        }
                    }
                    _ => {
                        println!("Error: {:?}", e);
                    }
                }
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
}

fn process_message(
    msg: Message,
    message_channel: UnboundedSender<ServerMessage>,
) -> ControlFlow<(), bool> {
    let mut got_start_mining_message = false;
    match msg {
        Message::Text(t) => {
            println!("{}", t);
        }
        Message::Binary(b) => {
            let message_type = b[0];
            match message_type {
                0 => {
                    if b.len() < 49 {
                        println!("Invalid data for Message StartMining");
                    } else {
                        let mut hash_bytes = [0u8; 32];
                        // extract 256 bytes (32 u8's) from data for hash
                        let mut b_index = 1;
                        for i in 0..32 {
                            hash_bytes[i] = b[i + b_index];
                        }
                        b_index += 32;

                        // extract 64 bytes (8 u8's)
                        let mut cutoff_bytes = [0u8; 8];
                        for i in 0..8 {
                            cutoff_bytes[i] = b[i + b_index];
                        }
                        b_index += 8;
                        let cutoff = u64::from_le_bytes(cutoff_bytes);

                        let mut nonce_start_bytes = [0u8; 8];
                        for i in 0..8 {
                            nonce_start_bytes[i] = b[i + b_index];
                        }
                        b_index += 8;
                        let nonce_start = u64::from_le_bytes(nonce_start_bytes);

                        let mut nonce_end_bytes = [0u8; 8];
                        for i in 0..8 {
                            nonce_end_bytes[i] = b[i + b_index];
                        }
                        let nonce_end = u64::from_le_bytes(nonce_end_bytes);

                        let msg =
                            ServerMessage::StartMining(hash_bytes, nonce_start..nonce_end, cutoff);

                        let _ = message_channel.send(msg);
                        got_start_mining_message = true;
                    }
                }
                1 => {
                    let msg = match bincode::deserialize(&b[1..]) {
                        Ok(value) => value,
                        Err(err) => {
                            // Handle the error, e.g., log it and return an error
                            eprintln!("Error deserializing: {}", err);
                            return ControlFlow::Break(());
                        }
                    };
                    let _ = message_channel.send(ServerMessage::PoolSubmissionResult(msg));
                }
                _ => {
                    println!("Failed to parse server message type");
                }
            }
        }
        Message::Ping(_) => {}
        Message::Pong(_) => {}
        Message::Close(v) => {
            println!("Got Close: {:?}", v);
            return ControlFlow::Break(());
        }
        _ => {}
    }

    ControlFlow::Continue(got_start_mining_message)
}

async fn submission_system(
    key: Option<Arc<Keypair>>,
    mut system_message_receiver: UnboundedReceiver<MessageSubmissionSystem>,
    socket_sender: Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    pubkey: Pubkey,
) {
    let mut best_diff = 0;
    while let Some(msg) = system_message_receiver.recv().await {
        match msg {
            MessageSubmissionSystem::Submission(thread_submission) => {
                if thread_submission.difficulty > best_diff {
                    best_diff = thread_submission.difficulty;

                    // Send results to the server
                    let message_type = 2u8; // 1 u8 - BestSolution Message
                    let best_hash_bin = thread_submission.d; // 16 u8
                    let best_nonce_bin = thread_submission.nonce.to_le_bytes(); // 8 u8

                    let mut hash_nonce_message = [0; 24];
                    hash_nonce_message[0..16].copy_from_slice(&best_hash_bin);
                    hash_nonce_message[16..24].copy_from_slice(&best_nonce_bin);

                    let mut bin_data = [0; 57];
                    bin_data[00..1].copy_from_slice(&message_type.to_le_bytes());
                    bin_data[01..17].copy_from_slice(&best_hash_bin);
                    bin_data[17..25].copy_from_slice(&best_nonce_bin);
                    bin_data[25..57].copy_from_slice(&pubkey.to_bytes());

                    let mut bin_vec = bin_data.to_vec();

                    if key.clone().is_some() {
                        let signature = key
                            .clone()
                            .unwrap()
                            .sign_message(&hash_nonce_message)
                            .to_string()
                            .as_bytes()
                            .to_vec();
                        bin_vec.extend(signature);
                    }

                    let mut message_sender = socket_sender.lock().await;
                    let _ = message_sender.send(Message::Binary(bin_vec)).await;
                    drop(message_sender);
                }
            }
            MessageSubmissionSystem::Reset => {
                best_diff = 0;

                // Sleep for 2 seconds to let the submission window open again
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            MessageSubmissionSystem::Finish => {
                return;
            }
        }
    }
}
