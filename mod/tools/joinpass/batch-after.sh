#!/bin/bash
# Interleaved: base rep 2 and fix1 reps 1-2 (MP), fix1 SP, fix1 pack joins.
cd /home/claude/joinpass-work
one() { # set ver arm mode rep [extra]
  local set=$1 ver=$2 arm=$3 mode=$4 rep=$5; shift 5
  local mv=$([ $ver = 189 ] && echo 1.8.9 || ([ $ver = 1211 ] && echo 1.21.11 || echo 26.3))
  local flag=$([ $ver = 189 ] && echo --noop || echo --perf)
  local tag=$([ $set = fixpack ] && echo fix1 || echo $set)
  local ja=""; [[ $arm == a* ]] && ja="--arctic jars/$tag-$mv.jar"
  local jp=""; [[ $arm == *p ]] && jp="--polarium jars/pol-base-$mv.jar"
  local m=""; [ $mode = sp ] && m="--mode sp"
  timeout 900 python3 run.py $ver $set-$ver-$arm-$mode$rep $flag $ja $jp $m "$@" 2>&1 | tail -1
}
for spec in "189 v" "189 a" "189 ap" "1211 v" "1211 a" "263 v" "263 a" "263 ap"; do
  set -- $spec
  [ $2 != v ] && one fix1 $1 $2 mp 1
  one base $1 $2 mp 2
done
for spec in "189 a" "189 ap" "1211 a" "263 a" "263 ap"; do set -- $spec; one fix1 $1 $2 mp 2; one fix1 $1 $2 sp 1; done
for r in 1 2; do one fixpack 1211 a mp $r --pack; one fixpack 263 a mp $r --pack; done
echo BATCH DONE
