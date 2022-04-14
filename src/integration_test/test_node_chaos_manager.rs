use crate::integration_test::test_model::{TestNode, TestNodes};
use crate::server::Router;
use crate::{Request, Response, Result};
use actix::{Actor, Context, Handler, ResponseFuture};
use futures_util::FutureExt;
use rand::{thread_rng, Rng};
use std::borrow::Borrow;
use std::collections::HashSet;
use std::ops::Range;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::{sleep, JoinHandle};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info};

pub struct TestNodeChaosManager {
    pub duration: Duration,
    pub test_nodes: Arc<Mutex<TestNodes>>,
    pub delay_sec_range: Range<u64>,
    pub node_ids_range: Range<usize>,
    is_stopped: Arc<Mutex<bool>>,
}

impl TestNodeChaosManager {
    pub fn new(
        test_nodes: Arc<Mutex<TestNodes>>,
        duration: Duration,
        delay_sec_range: Range<u64>,
        node_ids_range: Range<usize>,
    ) -> TestNodeChaosManager {
        TestNodeChaosManager {
            duration,
            test_nodes,
            delay_sec_range,
            node_ids_range,
            is_stopped: Arc::new(Mutex::new(false)),
        }
    }

    /// Start the chaos among all running nodes by running
    /// a chaos manager (thread) for each of them.
    ///
    /// The chaos manager will periodically start/stop a process for the node
    /// waiting for a random amount of time within the indicated range.
    /// There is a 50/50 chance for each chaos manager that the node remains
    /// in the same state (ex. started or stopped)
    pub fn run_chaos(&mut self) {
        for id in self.node_ids_range.start..self.node_ids_range.end {
            self.run_chaos_for_node(id);
        }
    }

    fn run_chaos_for_node(&mut self, node_id: usize) {
        let duration = self.duration.clone();
        let test_nodes = self.test_nodes.clone();
        let delay_sec_range = self.delay_sec_range.clone();
        let is_stopped = Arc::clone(&self.is_stopped);

        thread::spawn(move || {
            debug!("Starting chaos manager for node {}", node_id);

            let mut now = Instant::now();
            let mut rng = thread_rng();

            let mut elapsed = now.elapsed();
            while elapsed <= duration && !*is_stopped.lock().unwrap() {
                let delay = rng.gen_range(delay_sec_range.start, delay_sec_range.end);

                debug!("Wait for {} sec before managing node {}", delay, node_id);
                sleep(Duration::from_secs(delay));

                if rng.gen_range(0, 2) == 1 {
                    if test_nodes.lock().unwrap().is_running(node_id) {
                        debug!("stop the node {}", node_id);
                        test_nodes.lock().unwrap().kill_node(node_id);
                    } else {
                        debug!("start the node {}", node_id);
                        test_nodes.lock().unwrap().start_node(node_id);
                    }
                }
                elapsed = now.elapsed();
            }
            debug!("Stopping chaos manager for node {}", node_id);
        });
    }

    pub fn stop(&mut self) {
        debug!("stopping the chaos-monkey...");
        *self.is_stopped.lock().unwrap() = true;
        self.test_nodes.lock().unwrap().kill_all();
    }
}
