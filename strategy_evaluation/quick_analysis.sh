#!/bin/bash
# Quick analysis script for village model simulations

if [ $# -eq 0 ]; then
    echo "Usage: $0 <simulation_events.json>"
    exit 1
fi

FILE=$1
echo "=== Quick Analysis of $FILE ==="
echo

echo "Total events: $(jq 'length' $FILE)"
echo "Total trades: $(jq '[.[] | select(.event_type.type == "TradeExecuted")] | length' $FILE)"
echo "Total orders: $(jq '[.[] | select(.event_type.type == "OrderPlaced")] | length' $FILE)"
echo

echo "=== Trade Summary by Village ==="
jq -r '[.[] | select(.event_type.type == "TradeExecuted")] | 
    group_by(.village_id) | 
    map({
        village: .[0].village_id,
        trades: length,
        buys: map(select(.event_type.side == "Buy")) | length,
        sells: map(select(.event_type.side == "Sell")) | length
    }) | 
    .[] | 
    "\(.village): \(.trades) trades (\(.buys) buys, \(.sells) sells)"' $FILE

echo
echo "=== Resource Trading ==="
jq -r '[.[] | select(.event_type.type == "TradeExecuted")] | 
    group_by(.event_type.resource) | 
    map({
        resource: .[0].event_type.resource,
        count: length,
        total_qty: map(.event_type.quantity | tonumber) | add
    }) | 
    .[] | 
    "\(.resource): \(.count) trades, total quantity: \(.total_qty)"' $FILE

echo
echo "=== Final Populations ==="
jq -r '[.[] | select(.event_type.type == "PopulationUpdate")] | 
    group_by(.village_id) | 
    map({
        village: .[0].village_id,
        final_pop: .[-1].event_type.total_population
    }) | 
    .[] | 
    "\(.village): \(.final_pop) workers"' $FILE 2>/dev/null || echo "No population updates found"

echo
echo "=== Worker Deaths ==="
jq -r '[.[] | select(.event_type.type == "WorkerDied")] | 
    group_by(.event_type.cause) | 
    map({
        cause: .[0].event_type.cause,
        count: length
    }) | 
    .[] | 
    "\(.cause): \(.count) deaths"' $FILE 2>/dev/null || echo "No deaths found"