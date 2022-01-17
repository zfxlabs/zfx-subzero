#!/bin/sh

# run NODE_NUM BOOTSTRAP_NODE_NUM
run() {
    cargo run --bin node -- --listener-ip 127.0.0.1:123$1 --bootstrap-ip 127.0.0.1:123$2
}

# start $1 nodes in the background, logging to the same terminal
# use `killall node` or similar to stop them
start_same_term() {
       n=$1
        : $((n--))
        run 0 1 &
        echo Node 0 started, pid $! >&2
        for i in `eval echo {1..$n}`; do
            run $i 0 &
            echo Node $i started, pid $! >&2
        done
}

case $1 in
    0) run 0 1 ;;
    [1-9]) run $1 0 ;;
    n)
        start_same_term $2 ;;
    *)
        echo $0: bad argument
        exit 1
        ;;
esac
