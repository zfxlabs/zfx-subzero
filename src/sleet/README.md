# sleet

`sleet` is a transaction based consensus algorithm based on `Avalanche` which uses directed acylic graphs and multi-input conflict resolution. It is the primary consensus mechanism for mempools which use the UTXO model within the `zero.fx` network.

# Algorithm

```
let init = 
  AllT := () // The set of known transactions
  AllQ := () // The set of queried transactions

let on_receive_tx T =
  if T does not exist in AllT then
    if PbT == () then
      PbT := {T}, PbT.pref := T
      PbT.last := B, PbT.cnt := 0
    else
      PbT := PbT U {T}
    AllT := AllT U {T}, Cb := 0

let on_generate_tx data =
  edge := select_k_parents()
  T := Tx(data, edge)
  on_receive_tx T

let sleet_loop =
  while true do
    find T that satisfies { T exists in AllT and is not in the queried set Q }
    K := sample(N \ u, k)
    P := sum for v exists in K do query(v, T)
    if P >= alpha then
      Ct := 1
      for T' exists in AllT : T' <-* T_final do
        if conviction(T') > conviction(PbT'.pref) then
          PbT'.pref := T'
        if B' <> Pb'.last then
          PbT'.last := T', PbT'.cnt := 1
        else
          ++PbT'.cnt
    else
      for T' exists in AllT : T' <-* T_final do
        PbT'.cnt := 0;
    // otherwise Ct remains 0 forever
    Q = Q U {B}

// Sum all the chits of child transactions extending from T
let conviction T =
  sum all Ct for T <-* T'

let is_preferred T h =
  T == PbT.pref

// Check that the ancestry of T, T' are also preferred
let is_strongly_preferred T =
  forall T' which exist in AllT, T' <-* T : is_preferred T'

let is_accepted T =
  (forall T' which exist in T' <-* T : is_accepted(T') AND |PbT| == 1 AND PbT.cnt >= beta1) // early commitment
    OR (PbT.cnt >= beta2) // final

let on_query(peer, T) =
  on_receive_block T ;
  respond(peer, is_strongly_preferred T)
```
