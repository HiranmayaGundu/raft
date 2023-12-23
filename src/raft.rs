use std::sync::Mutex;
use std::time;

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
}

pub struct ConsensusModule {
    data: Mutex<ConsensusModuleData>,
}

impl ConsensusModule {
    pub fn new(id: u64, peer_ids: Vec<u64>) -> ConsensusModule {
        ConsensusModule {
            data: Mutex::new(ConsensusModuleData {
                id,
                peer_ids,
                current_term: 0,
                voted_for: None,
                state: CMState::Follower,
                election_reset_event: time::Instant::now(),
            }),
        }
    }
}
