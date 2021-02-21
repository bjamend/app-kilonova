use std::time::Instant;
use serde::{Serialize, Deserialize};




/**
 * A task, or side-effect, such as reporting, analysis, or data output
 */
#[derive(Clone, Serialize, Deserialize)]
pub struct RecurringTask {

    /// The number of times this task has been performed
    pub count: usize,

    /// The next simulation time at which this task is set to be performed
    pub next_time: f64,

    /// The last clock time when this task was performed
    #[serde(skip, default = "Instant::now")]
    pub last_performed: Instant,

    #[serde(skip, default = "usize::default")]
    pub count_this_run: usize,
}




/**
 * All the tasks that are used in this application
 */
#[derive(Clone, Serialize, Deserialize)]
pub struct Tasks {

    /// Write a snapshot of the full simulation
    pub write_checkpoint: RecurringTask,

    /// Output the primitive and geometric quantities for plotting and
    /// post-processing
    pub write_products: RecurringTask,

    /// Print the loop message
    pub iteration_message: RecurringTask,

    /// Summarize the simulation performance
    pub report_progress: RecurringTask,
}




// ============================================================================
impl RecurringTask {

    /**
     * Create a fresh recurring task which is first due at t = 0.0.
     */
    pub fn new(start_time: f64) -> Self {
        Self {
            count: 0,
            next_time: start_time,
            last_performed: Instant::now(),
            count_this_run: 0,
        }
    }

    /**
     * Mark the task as having just been performed, and schedule it to happen
     * again after the given time interval. Return the length of WALL time that
     * elapsed since the task was last performed.
     */
    pub fn advance(&mut self, interval: f64) -> f64 {
        let seconds = self.last_performed.elapsed().as_secs_f64();
        self.count += 1;
        self.count_this_run += 1;
        self.next_time += interval;
        self.last_performed = Instant::now();
        seconds
    }
}




// ============================================================================
impl Tasks {
    pub fn new(start_time: f64) -> Self {
        Self {
            write_checkpoint: RecurringTask::new(start_time),
            write_products: RecurringTask::new(start_time),
            iteration_message: RecurringTask::new(start_time),
            report_progress: RecurringTask::new(start_time),
        }
    }
}
