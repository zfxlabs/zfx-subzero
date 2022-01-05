# hail

`hail` is a block based consensus algorithm based on `Avalanche` which uses directed acylic graphs and `VRFs`. It is the primary consensus mechanism for all chains defined on within the `zero.fx` network.

`hail` is the fastest existing consensus algorithm in terms of time to reach finality on a block, able to achieve finality in less than a second.

# Algorithm

```
let init = 
  AllB := () // The set of known blocks
  AllQ := () // The set of queried blocks

let on_receive_block h B =
  if B does not exist in AllB then
    if Pb[h] == () then
      Pb[h] := {B}, Pb[h].pref := B
      Pb[h].last := B, Pb[h].cnt := 0
    else
      Pb[h] := Pb[h] U {B}
      // Whilst we have no confidence at this height, prefer the block with 
	  // lowest hash
      if Pb[h].cnt == 0 and lowest_hash B in Pb[h] then
        Pb[h].pref := B, Pb[h].last := B
    AllB := AllB U {B}, Cb := 0

let on_generate_block h data =
  edge := select_parent_at_height(h - 1)
  B := Block(data, edge)
  on_receive_block B

let hail_loop =
  while true do
    find B that satisfies { B exists in AllB and is not in the queried set Q }
    K := sample(N \ u, k)
    P := sum for v exists in K do query(v, B)
    if P >= alpha then
      Ct := 1
      for B' exists in AllB : B' <-* B_final do
        if conviction(B') > conviction(Pb'.pref) then
          Pb'.pref := B'
        if B' <> Pb'.last then
          Pb'.last := B', Pb'.cnt := 1
        else
          ++Pb'.cnt
    else
      for B' exists in AllB : B' <-* B_final do
        Pb'.cnt := 0
        reissue_transactions(B');
    // otherwise Ct remains 0 forever
    Q = Q U {B}

let select_parent_at_height h =
  Pb[h].pref

// Sum all the chits of child blocks extending from B
let conviction B =
  sum all Ct for B <-* B'

let is_preferred B h =
  B == Pb[h].pref

// Check that the ancestry of B, B' are also preferred
let is_strongly_preferred B =
  forall B' which exist in AllB, B' <-* B : is_preferred B'

let is_accepted B =
  (forall B' which exist in B' <-* B : is_accepted(B') AND |Pb| == 1 AND Pb.cnt >= beta1) // early commitment
    OR (Pb.cnt >= beta2) // final

// Note: It is assumed here that `B` was verified a priori
let on_query(peer, B) =
  on_receive_block B ;
  respond(peer, is_strongly_preferred B)
```

# VRF

VRFs are used in `hail` in order to provide five crucial attributes:
1. Fast leaderless selection of block producers. Selection of block producers is local, decided in constant time (within milliseconds) and verifiable by every participant.
2. Limited number of possible conflicts per height. `hail` uses `VRF`s to limit the number of possible conflicts arising per height to the `sqrt(N)` where `N` is the size of the network.
3. Tie-breaking between blocks conflicting at the same height. Instead of breaking ties lexicographically, ties are broken by using the lowest hash as output by the VRF.
4. Adaptive security. The next set of block producers cannot be known ahead of time.
5. Verifiably random blocks. Using `hail`, every block can be used as a pseudo random seed. There is no further need to use complicated on-chain randomness generation algorithms.

# Conflicts

In the original `Snow*` whitepaper, the definition of conflicts is error prone. Particulary because they are defined as being transitive, which can only apply as per the pseudocode when either single-input transactions are used, or blocks. Please see the multi-input conflict map defined for `sleet` for how the multi-input case is solved.

In the case of `hail`, conflicts are defined in terms of a conflict map specialised on `height`, which can be used to create transitive and equivalent conflict sets without ambiguity.
