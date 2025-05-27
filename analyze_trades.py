#!/usr/bin/env python3
import json
import sys

def analyze_trades(filename):
    with open(filename, 'r') as f:
        events = json.load(f)
    
    trades = []
    auction_clears = []
    village_orders = []
    allocations = []
    
    for event in events:
        event_type = event['event_type']
        if isinstance(event_type, dict):
            type_name = event_type.get('type')
            if type_name == 'TradeExecuted':
                trades.append(event)
            elif type_name == 'AuctionCleared':
                auction_clears.append(event)
            elif type_name == 'OrderPlaced':
                village_orders.append(event)
            elif type_name == 'WorkerAllocation':
                allocations.append(event)
    
    print(f"\nTotal events: {len(events)}")
    print(f"Total trades: {len(trades)}")
    print(f"Total auction clearings: {len(auction_clears)}")
    print(f"Total orders placed: {len(village_orders)}")
    
    # Analyze trades by village
    village_trade_summary = {}
    
    for trade in trades:
        village = trade['village_id']
        trade_data = trade['event_type']
        if village not in village_trade_summary:
            village_trade_summary[village] = {
                'buys': [],
                'sells': [],
                'total_spent': 0,
                'total_earned': 0
            }
        
        if trade_data['trade_type'] == 'Buy':
            village_trade_summary[village]['buys'].append(trade)
            village_trade_summary[village]['total_spent'] += float(trade_data['total_value'])
        else:
            village_trade_summary[village]['sells'].append(trade)
            village_trade_summary[village]['total_earned'] += float(trade_data['total_value'])
    
    print("\n=== Village Trade Summary ===")
    for village, summary in village_trade_summary.items():
        print(f"\n{village}:")
        print(f"  Buys: {len(summary['buys'])} trades")
        print(f"  Sells: {len(summary['sells'])} trades")
        print(f"  Total spent: {summary['total_spent']:.2f}")
        print(f"  Total earned: {summary['total_earned']:.2f}")
        print(f"  Net profit: {summary['total_earned'] - summary['total_spent']:.2f}")
        
        # Show first few trades
        if summary['buys']:
            print("  Sample buys:")
            for trade in summary['buys'][:3]:
                td = trade['event_type']
                print(f"    Tick {trade['tick']}: {td['quantity']} {td['resource']} @ {td['price']} = {td['total_value']}")
        
        if summary['sells']:
            print("  Sample sells:")
            for trade in summary['sells'][:3]:
                td = trade['event_type']
                print(f"    Tick {trade['tick']}: {td['quantity']} {td['resource']} @ {td['price']} = {td['total_value']}")
    
    # Analyze auction clearing prices
    print("\n=== Auction Clearing Prices ===")
    wood_prices = []
    food_prices = []
    
    for clear in auction_clears[:10]:  # First 10 ticks
        clear_data = clear['event_type']
        print(f"\nTick {clear['tick']}:")
        for resource, price in clear_data['clearing_prices'].items():
            print(f"  {resource}: {price}")
            if resource == 'Wood':
                wood_prices.append(float(price))
            elif resource == 'Food':
                food_prices.append(float(price))
    
    if wood_prices:
        print(f"\nAverage wood price: {sum(wood_prices)/len(wood_prices):.2f}")
    if food_prices:
        print(f"Average food price: {sum(food_prices)/len(food_prices):.2f}")
    
    # Analyze orders placed
    print("\n=== Orders by Village ===")
    village_order_summary = {}
    for order in village_orders:
        village = order['village_id']
        if village not in village_order_summary:
            village_order_summary[village] = {'bids': 0, 'asks': 0}
        
        order_data = order['event_type']
        if order_data['order_type'] == 'Bid':
            village_order_summary[village]['bids'] += 1
        else:
            village_order_summary[village]['asks'] += 1
    
    for village, summary in village_order_summary.items():
        print(f"\n{village}:")
        print(f"  Total bids: {summary['bids']}")
        print(f"  Total asks: {summary['asks']}")
    
    # Show sample orders
    print("\n=== Sample Orders (first 5) ===")
    for order in village_orders[:5]:
        order_data = order['event_type']
        print(f"Tick {order['tick']} - {order['village_id']}: {order_data['order_type']} {order_data['quantity']} {order_data['resource']} @ {order_data['price']}")
    
    # Analyze worker allocations
    print("\n=== Worker Allocation Patterns ===")
    for village in ['village_a', 'village_b']:
        village_allocs = [a for a in allocations if a['village_id'] == village][:5]
        if village_allocs:
            print(f"\n{village} (first 5 ticks):")
            for alloc in village_allocs:
                a = alloc['event_type']
                print(f"  Tick {alloc['tick']}: Food={a['food_workers']}, Wood={a['wood_workers']}, Construction={a['construction_workers']}, Idle={a['idle_workers']}")

if __name__ == "__main__":
    if len(sys.argv) > 1:
        analyze_trades(sys.argv[1])
    else:
        analyze_trades("strategy_evaluation/trading_vs_balanced.json")