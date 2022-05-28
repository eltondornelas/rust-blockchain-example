use crate::{App, Block};
use libp2p::{
    floodsub::{Floodsub, FloodsubEvent, Topic},
    identity,
    mdns::{Mdns, MdnsEvent},
    swarm::{NetworkBehaviourEventProcess, Swarm},
    NetworkBehaviour, PeerId,
};
use log::{error, info};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

pub static KEYS: Lazy<identity::Keypair> = Lazy::new(identity::Keypair::generate_ed25519);
pub static PEER_ID: Lazy<PeerId> = Lazy::new(|| PeerId::from(KEYS.public()));
pub static CHAIN_TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("chains"));
pub static BLOCK_TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("blocks"));
// topics are channels to subscribe to

#[derive(Debug, Serialize, Deserialize)]
pub struct ChainResponse {
    // we'll expect if someone sends us their local blockchain and use to send them our local chain
    pub receiver: String,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalChainRequest {
    /* triggers the interaction. If we send a LocalChainRequest with the peer_id of another node in the system,
     * this will trigger that they send us their chain back.
     */
    pub from_peer_id: String,
}

/* To handle incoming messages, lazy initialization, and keyboard-input by the client’s user,
 * we define the EventType enum, which will help us send events across the application to keep our
 * application state in sync with incoming and outgoing network traffic. */
pub enum EventType {
    LocalChainResponse(ChainResponse),
    Input(String),
    Init,
}

/* the core of the P2P functionality, which implements NetworkBehaviour */
#[derive(NetworkBehaviour)]
pub struct AppBehaviour {
    pub floodsub: Floodsub, // publish/subscribe protocol, for communication between the nodes
    pub mdns: Mdns, // will enable us to automatically find other nodes on our local network (but not outside of it)
    #[behaviour(ignore)]
    pub response_sender: mpsc::UnboundedSender<ChainResponse>,
    #[behaviour(ignore)]
    pub init_sender: mpsc::UnboundedSender<bool>,
    #[behaviour(ignore)]
    pub app: App,
}

impl AppBehaviour {
    pub async fn new(
        app: App,
        response_sender: mpsc::UnboundedSender<ChainResponse>,
        init_sender: mpsc::UnboundedSender<bool>,
    ) -> Self {
        let mut behaviour = Self {
            app,
            floodsub: Floodsub::new(*PEER_ID),
            mdns: Mdns::new(Default::default())
                .await
                .expect("can create mdns"),
            response_sender,
            init_sender,
        };
        behaviour.floodsub.subscribe(CHAIN_TOPIC.clone());
        behaviour.floodsub.subscribe(BLOCK_TOPIC.clone());

        behaviour
    }
}

/* libp2p’s concept for implementing a decentralized network stack. */
impl NetworkBehaviourEventProcess<MdnsEvent> for AppBehaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(discovered_list) => {
                // if new node is discovered, we add to FloodSub list of nodes so we can communicate.
                for (peer, _addr) in discovered_list {
                    self.floodsub.add_node_to_partial_view(peer);
                }
            }
            MdnsEvent::Expired(expired_list) => {
                // once it expires, we remove it again
                for (peer, _addr) in expired_list {
                    if !self.mdns.has_node(&peer) {
                        self.floodsub.remove_node_from_partial_view(&peer);
                    }
                }
            }
        }
    }
}

// incoming event handler
impl NetworkBehaviourEventProcess<FloodsubEvent> for AppBehaviour {
    fn inject_event(&mut self, event: FloodsubEvent) {
        if let FloodsubEvent::Message(msg) = event {
            // check whether the payload fits any of our expected data
            if let Ok(resp) = serde_json::from_slice::<ChainResponse>(&msg.data) {
                // If it’s a ChainResponse, it means we got sent a local blockchain by another node.
                if resp.receiver == PEER_ID.to_string() {
                    info!("checking the receiver. {}", resp.receiver);
                    /* We check wether we’re actually the receiver of said piece of data and,
                     * if so, log the incoming blockchain and attempt to execute our consensus.
                     * If it’s valid and longer than our chain, we replace our chain with it.
                     * Otherwise, we keep our own chain. */
                    info!("Response from {}:", msg.source);
                    resp.blocks.iter().for_each(|r| info!("{:?}", r));

                    // attempt to execute our consensus
                    self.app.blocks = self.app.choose_chain(self.app.blocks.clone(), resp.blocks);
                }
            } else if let Ok(resp) = serde_json::from_slice::<LocalChainRequest>(&msg.data) {
                info!("sending local chain to {}", msg.source.to_string());
                let peer_id = resp.from_peer_id;
                /* we check whether we’re the ones they want the chain from, checking the from_peer_id
                 * if so, we simply send them a JSON version of our local blockchain. */
                if PEER_ID.to_string() == peer_id {
                    if let Err(e) = self.response_sender.send(ChainResponse {
                        blocks: self.app.blocks.clone(),
                        receiver: msg.source.to_string(),
                    }) {
                        error!("error sending response via channel, {}", e);
                    }
                }
            } else if let Ok(block) = serde_json::from_slice::<Block>(&msg.data) {
                /* Finally, if it’s a Block that’s incoming, that means someone else mined a block
                 * and wants us to add it to our local chain.
                 * We check whether the block is valid and, if it is, add it. */
                info!("received new block from {}", msg.source.to_string());
                self.app.try_add_block(block);
            }
        }
    }
}
