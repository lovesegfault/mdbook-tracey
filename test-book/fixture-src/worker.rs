// r[impl obs.log.batch-64-100ms]
pub fn push_log_line() {}

// r[verify obs.log.batch-64-100ms]
#[test]
fn batches_up_to_64() {}

// r[verify obs.log.periodic-flush]
#[test]
fn flushes_every_30s() {}
