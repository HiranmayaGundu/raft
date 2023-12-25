use log::debug;
use raft_service::raft_server::Raft;
use raft_service::{AppendEntriesArgs, AppendEntriesReply, RequestVoteArgs, RequestVoteReply};
use tonic::{Request, Response, Status};

pub mod raft_service {
    tonic::include_proto!("raft"); // The string specified here must match the proto package name
}
mod raft;

pub struct RaftService {
    raft: raft::ConsensusModule,
}

impl RaftService {
    pub async fn new(
        server_id: u64,
        peer_ids: Vec<u64>,
        peer_clients: Vec<crate::raft_service::raft_client::RaftClient<tonic::transport::Channel>>,
    ) -> Self {
        let raft =
            raft::ConsensusModule::new(server_id, peer_ids.clone(), peer_clients.clone()).await;
        RaftService { raft }
    }
}

#[tonic::async_trait]
impl Raft for RaftService {
    async fn request_vote(
        &self,
        request: Request<RequestVoteArgs>,
    ) -> Result<Response<RequestVoteReply>, Status> {
        debug!("Got a request: {:?}", request);

        let reply = self.raft.request_vote(request.into_inner()).await;

        match reply {
            Some(reply) => {
                let reply = RequestVoteReply {
                    term: reply.term,
                    vote_granted: reply.vote_granted,
                };
                Ok(Response::new(reply))
            }
            None => Err(Status::unavailable("")),
        }
    }

    async fn append_entries(
        &self,
        request: Request<AppendEntriesArgs>,
    ) -> Result<Response<AppendEntriesReply>, Status> {
        debug!("Got a request: {:?}", request);

        let reply = self.raft.append_entries(request.into_inner()).await;
        match reply {
            Some(reply) => {
                let reply = AppendEntriesReply {
                    term: reply.term,
                    success: reply.success,
                };
                Ok(Response::new(reply))
            }
            None => Err(Status::unavailable("")),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let addr = "[::1]:50051".parse()?;
    // let greeter = RaftService::new()

    // Server::builder()
    //     .add_service(RaftServer::new(greeter))
    //     .serve(addr)
    //     .await?;

    Ok(())
}
