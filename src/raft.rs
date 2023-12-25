use async_recursion::async_recursion;
use log::debug;
use raft_service::raft_client::RaftClient;
use rand::Rng;
use tokio::sync::Mutex;
use tokio::time;

use crate::raft_service;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum CMState {
    // The server is a follower of the leader.
    Follower,
    // The server is a candidate in an election.
    // Only happens when the server thinks
    // an election is happening.
    Candidate,
    // The server is the leader.
    // There is only one leader at a time.
    // The leader is the only one that responds
    // to client requests.
    Leader,
    // The server is dead.
    Dead,
}

struct LogEntry {
    // The term of the entry.
    term: u64,
    // The command of the entry.
    command: String,
}

struct ConsensusModuleData {
    // The id of this server.
    id: u64,
    // The ids of the other servers.
    peer_ids: Vec<u64>,
    // The term that the server knows about.
    // starts at zero, increases monotonically.
    // all the servers should be on the highest current term.
    current_term: u64,
    // The id of the server that this server voted for in the current term.
    voted_for: Option<u64>,
    // The state that the server is in.
    state: CMState,
    // The time that the server last heard from the leader.
    election_reset_event: time::Instant,
    // The log of commands that the server needs to keep track of.
    log: Vec<LogEntry>,

    peer_clients: Vec<RaftClient<tonic::transport::Channel>>,
}
/// A Raft server.
/// I'm forced to do this now where the type is cm.data.id etc
/// But i'd rather it just be cm.id etc.
/// Don't know how to do that yet, while still locking it.
pub struct ConsensusModule {
    data: Mutex<ConsensusModuleData>,
}

impl ConsensusModule {
    pub async fn new(
        id: u64,
        peer_ids: Vec<u64>,
        peer_clients: Vec<RaftClient<tonic::transport::Channel>>,
    ) -> ConsensusModule {
        let cm = ConsensusModule {
            data: Mutex::new(ConsensusModuleData {
                id,
                peer_ids,
                current_term: 0,
                voted_for: None,
                state: CMState::Follower,
                election_reset_event: time::Instant::now(),
                log: vec![],
                peer_clients,
            }),
        };

        cm.data.lock().await.election_reset_event = time::Instant::now();
        cm.run_election_timer().await;
        return cm;
    }

    fn election_timeout(&self) -> time::Duration {
        let mut rng = rand::thread_rng();
        time::Duration::from_millis(150 + rng.gen_range(0..150))
    }

    async fn become_follower(&self, term: u64) {
        let mut data = self.data.lock().await;
        data.state = CMState::Follower;
        data.current_term = term;
        data.voted_for = None;
        data.election_reset_event = time::Instant::now();
    }

    async fn leader_send_heartbeats(&self) {
        let data = self.data.lock().await;
        if data.state != CMState::Leader {
            return;
        }
        let current_term = data.current_term;
        let mut peer_clients = data.peer_clients.clone();
        let mut peer_ids = data.peer_ids.clone();
        let mut peer_clients_iter = peer_clients.iter_mut();
        let mut peer_ids_iter = peer_ids.iter_mut();
        let mut peer_client = peer_clients_iter.next();
        let mut peer_id = peer_ids_iter.next();
        while peer_client.is_some() && peer_id.is_some() {
            let peer_c = peer_client.unwrap();
            let append_entries_args = raft_service::AppendEntriesArgs {
                term: data.current_term,
                leader_id: data.id,
                prev_log_index: 0,
                prev_log_term: 0,
                entries: vec![],
            };
            let reply = peer_c
                .append_entries(append_entries_args)
                .await
                .unwrap()
                .into_inner();
            if reply.term > current_term {
                self.become_follower(reply.term).await;
                return;
            }
            peer_client = peer_clients_iter.next();
            peer_id = peer_ids_iter.next();
        }
    }

    async fn start_leader(&self) {
        let mut data = self.data.lock().await;
        data.state = CMState::Leader;

        let mut interval = time::interval(time::Duration::from_millis(50));
        loop {
            interval.tick().await;
            if data.state != CMState::Leader {
                return;
            }
            self.leader_send_heartbeats().await;
        }
    }

    pub async fn stop(&self) {
        let mut data = self.data.lock().await;
        data.state = CMState::Dead;
    }

    pub async fn append_entries(
        &self,
        args: raft_service::AppendEntriesArgs,
    ) -> Option<raft_service::AppendEntriesReply> {
        let mut data = self.data.lock().await;
        if data.state == CMState::Dead {
            return None;
        }
        debug!("{} got an append entries request", data.id);
        if args.term > data.current_term {
            debug!(
                "{} got an append entries request with a higher term",
                data.id
            );
            self.become_follower(args.term).await;
        }
        if data.current_term == args.term {
            if data.state != CMState::Follower {
                self.become_follower(args.term).await;
            }
            data.election_reset_event = time::Instant::now();
            return Some(raft_service::AppendEntriesReply {
                term: data.current_term,
                success: true,
            });
        } else {
            return Some(raft_service::AppendEntriesReply {
                term: data.current_term,
                success: false,
            });
        }
    }

    pub async fn request_vote(
        &self,
        args: raft_service::RequestVoteArgs,
    ) -> Option<raft_service::RequestVoteReply> {
        let mut data = self.data.lock().await;
        if data.state == CMState::Dead {
            return None;
        }
        debug!("{} got a request vote request", data.id);
        if args.term > data.current_term {
            debug!("{} got a request vote request with a higher term", data.id);
            self.become_follower(args.term).await;
        }
        if data.current_term == args.term
            && (data.voted_for.is_none() || data.voted_for.unwrap() == args.candidate_id)
        {
            debug!("{} voted for {}", data.id, args.candidate_id);
            data.voted_for = Some(args.candidate_id);
            return Some(raft_service::RequestVoteReply {
                term: data.current_term,
                vote_granted: true,
            });
        } else {
            debug!("{} did not vote for {}", data.id, args.candidate_id);
            return Some(raft_service::RequestVoteReply {
                term: data.current_term,
                vote_granted: false,
            });
        }
    }

    #[async_recursion]
    async fn start_election(&self, data: &mut tokio::sync::MutexGuard<'_, ConsensusModuleData>) {
        data.state = CMState::Candidate;
        data.current_term += 1;
        let saved_current_term = data.current_term;
        data.voted_for = Some(data.id);
        data.election_reset_event = time::Instant::now();

        let mut votes_received: u32 = 1;

        let request_vote_args = raft_service::RequestVoteArgs {
            term: data.current_term,
            candidate_id: data.id,
            last_log_index: 0,
            last_log_term: 0,
        };

        for peer_id in data.peer_ids.clone() {
            let client = data.peer_clients.get_mut(peer_id as usize);

            let reply = match client {
                Some(client) => {
                    let reply = client
                        .request_vote(request_vote_args.clone())
                        .await
                        .unwrap()
                        .into_inner();
                    reply
                }
                None => panic!("No peer client found"),
            };

            if data.state != CMState::Candidate {
                return;
            }
            if reply.term > saved_current_term {
                self.become_follower(reply.term).await;
                return;
            }
            if reply.vote_granted {
                votes_received += 1;
                if votes_received > data.peer_ids.len() as u32 / 2 {
                    self.start_leader().await;
                    return;
                }
            }
        }

        self.run_election_timer().await;
    }

    pub async fn run_election_timer(&self) {
        let timeout_duration = self.election_timeout();
        let data = self.data.lock().await;
        let term_started = data.current_term;
        drop(data);
        debug!(
            "election timer started {:?}, for term {}",
            timeout_duration, term_started
        );
        let mut interval = time::interval(timeout_duration);
        loop {
            interval.tick().await;
            let mut data = self.data.lock().await;

            if data.state != CMState::Candidate && data.state != CMState::Follower {
                debug!("election timer stopped for term {:?}", data.state);
                return;
            }

            if term_started != data.current_term {
                debug!(
                    "in election timer term changed from {} to {}",
                    term_started, data.current_term
                );
                return;
            }

            let elapsed = time::Instant::now().duration_since(data.election_reset_event);
            if elapsed >= timeout_duration {
                self.start_election(&mut data).await;
                debug!("election timer timed out for term {}", data.current_term);
                break;
            }
        }
    }
}
