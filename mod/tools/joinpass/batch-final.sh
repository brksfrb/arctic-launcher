#!/bin/bash
# Final A/B at -Xmx2g: vanilla, Arctic base (origin/main), Arctic final (perf/join-pass), each +/- Polarium.
# Interleaved by rep so drift hits every arm alike; one server+client pair at a time.
cd /home/claude/joinpass-work
one() { # ver arm(v|ab|af|apb|apf) mode rep
  local ver=$1 arm=$2 mode=$3 rep=$4
  local mv=$([ $ver = 189 ] && echo 1.8.9 || ([ $ver = 1211 ] && echo 1.21.11 || echo 26.3))
  local flag=$([ $ver = 189 ] && echo --noop || echo --perf)
  local ja="" jp=""
  case $arm in
    ab) ja="--arctic jars/base-$mv.jar";;
    af) ja="--arctic jars/final-$mv.jar";;
    apb) ja="--arctic jars/base-$mv.jar"; jp="--polarium jars/pol-base-$mv.jar";;
    apf) ja="--arctic jars/final-$mv.jar"; jp="--polarium jars/pol-base-$mv.jar";;
  esac
  local m=""; [ $mode = sp ] && m="--mode sp"
  timeout 900 python3 run.py $ver fin-$ver-$arm-$mode$rep $flag $ja $jp $m 2>&1 | tail -1
  pkill -f "joinagent.jar=/home/claude/joinpass-work/runs/fin-" 2>/dev/null; sleep 2
}
for rep in 1 2; do
  for ver in 189 1211 263; do
    arms="v ab af"; [ $ver != 1211 ] && arms="v ab af apb apf"
    for arm in $arms; do one $ver $arm mp $rep; done
  done
done
for ver in 189 1211 263; do for arm in v ab af; do one $ver $arm sp 1; done; done
echo BATCH DONE
