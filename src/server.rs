use std::vec;

use raft_service::raft_client::RaftClient;
use raft_service::raft_server::{Raft, RaftServer};
use raft_service::{AppendEntriesArgs, AppendEntriesReply, RequestVoteArgs, RequestVoteReply};
use tokio::sync::Mutex;
use tonic::transport::Channel;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

pub mod raft_service {
    tonic::include_proto!("raft"); // The string specified here must match the proto package name
}
mod raft;

struct RaftServiceData {
    server_id: u64,
    peer_ids: Vec<u64>,
    // consensus_module: raft::ConsensusModule<'a>,
    peer_clients: Vec<RaftClient<Channel>>,
}

pub struct RaftService {
    data: Mutex<RaftServiceData>,
}

impl RaftService {
    pub fn new(
        &self,
        server_id: u64,
        peer_ids: Vec<u64>,
        peer_clients: Vec<RaftClient<Channel>>,
    ) -> Self {
        let peer_ids_clone = peer_ids.clone();
        RaftService {
            data: Mutex::new(RaftServiceData {
                server_id,
                peer_ids,
                // consensus_module: raft::ConsensusModule::new(server_id, peer_ids_clone, &mut &self),
                peer_clients,
            }),
        }
    }

    pub async fn request_vote(&mut self, peer: usize, args: RequestVoteArgs) -> RequestVoteReply {
        let mut data = self.data.lock().await;
        let peer_client = data.peer_clients.get_mut(peer);
        match peer_client {
            Some(peer_client) => {
                let reply = peer_client.request_vote(args).await.unwrap().into_inner();
                reply
            }
            None => panic!("No peer client found"),
        }
    }
}

#[tonic::async_trait]
impl Raft for RaftService {
    async fn request_vote(
        &self,
        request: Request<RequestVoteArgs>, // Accept request of type HelloRequest
    ) -> Result<Response<RequestVoteReply>, Status> {
        // Return an instance of type HelloReply
        println!("Got a request: {:?}", request);

        let reply = RequestVoteReply {
            term: 0,
            vote_granted: false,
        };
        Ok(Response::new(reply)) // Send back our formatted greeting
    }

    async fn append_entries(
        &self,
        request: Request<AppendEntriesArgs>, // Accept request of type HelloRequest
    ) -> Result<Response<AppendEntriesReply>, Status> {
        // Return an instance of type HelloReply
        println!("Got a request: {:?}", request);

        let reply = AppendEntriesReply {
            term: 0,
            success: false,
        };
        Ok(Response::new(reply)) // Send back our formatted greeting
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let addr = "[::1]:50051".parse()?;
    // let greeter = RaftService::new(&self, server_id, peer_ids, peer_clients)

    // Server::builder()
    //     .add_service(RaftServer::new(greeter))
    //     .serve(addr)
    //     .await?;

    Ok(())
}
