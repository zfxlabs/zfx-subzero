# Integration tests
This file describes the list of functional and non-functional integration tests of zfx-subzero. 
These tests verify integration among components of the network, node bootstrapping and local performance of different operations (ex. balance transfer).

## How to run

`cargo test --features integration_tests`

## Integration test suite

The current test suite includes the following tests:

Functional integration tests:
* **test_send_cell** - Transfer balance from one node to another and validate its content
* **test_send_cell_with_modified_owner** - Modify owner of all outputs of the cell and try to send it Then verify that the transfer was not successful
* **test_send_same_cell_twice** - Transfer the same balance 2 times and validate that it fails the second time
* **test_send_cell_to_recipient_with_random_key** - Transfer balance with modified recipient public key and verify that transaction fails
* **test_send_cell_to_non_existing_recipient** - Transfer balance to non-existing recipient and check it was successful because a transfer can be made to any valid public key
* **test_spend_unspendable_cell** - Transfer balance to un-spendable cell, the one which had been already spent earlier and validate that it didn't go through
* **test_send_cell_when_has_faulty_node** - Try to send a transfer when 1 node is down and validate that transfer was not successful
* **test_send_cell_to_recipient_with_non_existing_coinbase** - Try to transfer a non-existing Coinbase and validate that it was rejected
* **test_successful_block_generation** - Make several transfers and verify that a block is generated with a valid set of accepted cells
  _(the test is temporary disabled until the hail component is fully finished)_

Non-Functional integration tests:
* **run_stress_test** - Run stress test by transferring valid cells among 3 nodes in parallel. Verifies that all cells were transferred and stored in 'sleet'.
  Verifies transfer and remaining balance in all nodes. Verifies that blocks contains accepted cells in all 3 nodes are same and unique.
* **run_stress_test_with_failed_transfers** - Transfer valid and invalid cells in parallel from one node to another.
  Validate that valid cells were transferred successfully and invalid cells are ignored.
* **run_node_communication_stress_test** - Run or stop n-number of nodes periodically for some time and verify the status of each node - number of peers,
validators and its weight
* **run_stress_test_with_chaos** - Transfer valid cells from one node to another when a random node can stop/start periodically.
  The random node which stops must not affect the stability and reaching consensus for transactions. Verifies that all cells were transferred successfully.
* **run_cell_transfer_benchmark_test** - Run a performance test involving parallel cell transfers among 3 nodes.
  Records time of each cell transfer and verifies min, max and avg time.
  _NOTE: the performance of cell transfers is run on local machine which varies in hardware thus the timings can be different. 
   This test is intended to capture a performance degradation on local machine._