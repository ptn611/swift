use std::time::UNIX_EPOCH;

use drift_rs::types::MarketType;
use solana_pubkey::Pubkey;

use crate::types::messages::IncomingSignedMessage;

#[derive(PartialEq, Debug, strum_macros::AsRefStr)]
pub enum WsError {
    /// Client tried to do a protected action while unauthenticated
    Unauthenticated,
    /// Client failed to authenticate its challenge
    FailedChallenge,
    /// Unknown topic
    UnknownTopic(String),
    /// Server couldn't handle the message backpressure
    /// in a timely manner
    Backpressure,
    /// Sending Ws update to client failed
    SendFailed,
    /// Websocket closed unexpectedly
    SocketClosed,
    /// Closed for some Ws protocol reason
    Protocol,
    /// Internal channel closed
    ChannelClosed,
    /// Invalid client message
    BadMessage,
    /// Failed Ws handshake
    Handshake,
}

/// Current unix timestamp (ms)
pub fn unix_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[derive(Clone)]
pub struct RequestContext {
    pub recv_ts: u64,
    pub log_prefix: String,
    pub market_index: u16,
    pub market_type: &'static str,
    pub taker_authority: Pubkey,
    pub order_uuid: String,
}

impl RequestContext {
    pub fn from_incoming_message(msg: &IncomingSignedMessage) -> Result<Self, ()> {
        let recv_ts = unix_now_ms();
        let info = msg.order().info(&msg.taker_authority);
        let market_index = info.order_params.market_index;
        let market_type = match info.order_params.market_type {
            MarketType::Perp => "perp",
            MarketType::Spot => "spot",
        };
        match core::str::from_utf8(&info.uuid) {
            Ok(order_uuid) => {
                let taker_authority = if msg.taker_authority != Pubkey::default() {
                    msg.taker_authority
                } else {
                    msg.taker_pubkey
                };

                Ok(Self {
                    market_index,
                    market_type,
                    recv_ts,
                    log_prefix: format!(
                        "[order uuid={order_uuid} market={market_type}:{market_index} taker={taker_authority}]"
                    ),
                    taker_authority,
                    order_uuid: order_uuid.to_string(),
                })
            }
            Err(_) => {
                let taker_authority = if msg.taker_authority != Pubkey::default() {
                    msg.taker_authority
                } else {
                    msg.taker_pubkey
                };
                log::info!(
                    target: "swift",
                    "[order market={market_type}:{market_index} taker={taker_authority}] uuid invalid utf-8"
                );
                Err(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // V-IMPL FIX r169: build payload theo layout hien hanh + ky that (thay hex cu)
    #[test]
    fn from_incoming_message_valid_utf8_uuid() {
        use anchor_lang::AnchorSerialize;
        use base64::Engine;
        use drift_rs::drift_idl::types::{
            OrderParams, OrderTriggerCondition as IdlTriggerCondition, OrderType as IdlOrderType,
            PositionDirection as IdlDirection, PostOnlyParam as IdlPostOnly,
            SignedMsgOrderParamsMessage,
        };

        let m = SignedMsgOrderParamsMessage {
            signed_msg_order_params: OrderParams {
                order_type: IdlOrderType::Market,
                market_type: drift_rs::drift_idl::types::MarketType::Perp,
                direction: IdlDirection::Short,
                user_order_id: 0,
                base_asset_amount: 2_000_000,
                price: 0,
                market_index: 2,
                reduce_only: false,
                post_only: IdlPostOnly::None,
                bit_flags: 0,
                max_ts: None,
                trigger_price: None,
                trigger_condition: IdlTriggerCondition::Above,
                offset: None,
                offset_type: None,
                auction_duration: Some(50),
                auction_start_price: Some(2_102_419_643),
                auction_end_price: Some(2_081_603_607),
            },
            sub_account_id: 2,
            slot: 369_631_527,
            uuid: *b"s8luJLZe",
            take_profit_order_params: None,
            stop_loss_order_params: None,
            max_margin_ratio: Some(u16::MAX),
            builder_idx: None,
            builder_fee_tenth_bps: None,
            isolated_position_deposit: None,
        };
        let mut buf = b"smsgv002".to_vec();
        AnchorSerialize::serialize(&m, &mut buf).unwrap();
        let payload_hex = faster_hex::hex_string(&buf);

        let kp = solana_keypair::Keypair::new();
        let sig = solana_signer::Signer::sign_message(&kp, payload_hex.as_bytes());
        let sig_b64 = base64::prelude::BASE64_STANDARD.encode(sig.as_ref());
        let pk = solana_signer::Signer::pubkey(&kp).to_string();

        let json = format!(
            r#"{{"market_index":2,"market_type":"perp","message":"{payload_hex}","signature":"{sig_b64}","signing_authority":"{pk}","taker_pubkey":"{pk}"}}"#
        );
        let msg: IncomingSignedMessage = serde_json::from_str(&json).expect("deserialize");
        let ctx = RequestContext::from_incoming_message(&msg).expect("valid utf8 uuid");
        assert_eq!(ctx.order_uuid, "s8luJLZe");
        assert_eq!(ctx.market_index, 2);
        assert_eq!(ctx.market_type, "perp");
    }
}
