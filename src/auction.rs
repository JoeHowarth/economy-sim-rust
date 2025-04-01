#![allow(dead_code, unused_imports)]

use std::collections::HashMap;

use BidAsk::{Ask, Bid};

use crate::fp::{Fp, dec, fp};

type Actor = u16;
type Price = Fp;
type Qty = Fp;
type Resource = &'static str;
type Curve = Vec<(Order, Qty)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BidAsk {
    Bid,
    Ask,
}

impl BidAsk {
    fn outflow_sign(&self) -> Fp {
        match self {
            Bid => fp(1),
            Ask => fp(-1),
        }
    }

    fn from_sign(x: Fp) -> Self {
        if x > fp(0) { Bid } else { Ask }
    }
}

#[derive(Clone, Copy, Debug)]
struct Order {
    actor: Actor,
    res: Resource,
    side: BidAsk,
    limit: Price,
    qty: Qty,
}

fn run_auction(orders: HashMap<Actor, Vec<Order>>, mut wallets: HashMap<Actor, Fp>) {
    let mut resource_books = group_orders(&orders);

    for _ in 0..10 {
        build_curves(&mut resource_books);

        let clearings = resource_books
            .iter() //
            .filter_map(|(res, (bids, asks))| {
                find_clearing_price(bids, asks).map(|c| (*res, c)) //
            })
            .collect::<Vec<_>>();

        println!("Clearings: {:?}", clearings);

        let any_pruned = net_outflows(&mut wallets, &mut resource_books, clearings.into_iter());
        if !any_pruned {
            break;
        }
    }
}

fn group_orders(orders: &HashMap<Actor, Vec<Order>>) -> HashMap<Resource, (Curve, Curve)> {
    let mut resource_books: HashMap<Resource, (Curve, Curve)> = HashMap::default();
    for order in orders.values().flatten() {
        let (bids, asks) = resource_books.entry(order.res).or_default();

        let val = (*order, fp(0));
        match order.side {
            Bid => bids.push(val),
            Ask => asks.push(val),
        }
    }
    resource_books
}

fn build_curves(resource_books: &mut HashMap<Resource, (Curve, Curve)>) {
    for (_, (bids, asks)) in resource_books.iter_mut() {
        // Sort
        bids.sort_by_key(|(order, _)| order.limit);
        asks.sort_by_key(|(order, _)| order.limit);

        // Compute cumulative Qty of bids
        let mut acc = fp(0);
        let len = bids.len();
        for i in 1..=len {
            let (order, cum) = &mut bids[len - i];
            *cum = order.qty + acc;
            acc = *cum;
        }

        // Compute cumulative Qty of asks
        let mut acc = fp(0);
        for (order, cum) in asks {
            *cum = order.qty + acc;
            acc = *cum;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Clearing {
    price: Price,
    matched_qty: Qty,
    bid_idx: usize,
    ask_idx: usize,
}

fn find_clearing_price<'a>(bids: &'a Curve, asks: &'a Curve) -> Option<Clearing> {
    println!(
        "bids: {:?}",
        bids.iter()
            .map(|(order, cum)| (order.limit, cum))
            .collect::<Vec<_>>()
    );
    println!(
        "asks: {:?}",
        asks.iter()
            .map(|(order, cum)| (order.limit, cum))
            .collect::<Vec<_>>()
    );

    if bids.is_empty() || asks.is_empty() {
        return None;
    }
    let mut bid_idx =
        match bids.binary_search_by_key(&asks.first().unwrap().0.limit, |(order, _)| order.limit) {
            Ok(idx) => idx,
            Err(idx) => idx,
        };
    println!("bid_idx: {}", bid_idx);

    // No overlap
    if bid_idx == bids.len() {
        return None;
    }
    println!("bid_idx: {}", bid_idx);

    // (volume, bid_idx, ask_idx);
    let mut best = Clearing {
        price: fp(0),
        matched_qty: fp(0),
        bid_idx,
        ask_idx: 0,
    };
    for (ask_idx, (ask, ask_cum)) in asks.iter().enumerate() {
        // find matching bid
        while bid_idx < bids.len() && dbg!(bids[bid_idx].0.limit) < dbg!(ask.limit) {
            bid_idx += 1;
        }
        if bid_idx == bids.len() {
            break;
        }
        let (_, bid_cum) = &bids[bid_idx];

        println!("bid_idx: {}, ask_idx: {}", bid_idx, ask_idx);
        println!("bid_cum: {}, ask_cum: {}", bid_cum, ask_cum);

        let matched_qty = *bid_cum.min(ask_cum);
        println!("matched_qty: {}", matched_qty);
        if matched_qty > best.matched_qty {
            best = Clearing {
                price: ask.limit,
                matched_qty,
                bid_idx,
                ask_idx,
            };
            println!("new best: {:?}", best);
        }
    }

    Some(best)
}

fn net_outflows(
    wallets: &mut HashMap<Actor, Fp>,
    resource_books: &mut HashMap<Resource, (Curve, Curve)>,
    clearings: impl Iterator<Item = (Resource, Clearing)>,
) -> bool {
    println!("\nResource_books: {:?}", resource_books);

    #[derive(Debug)]
    struct Fill {
        res: Resource,
        idx: usize,
        net: Fp,
    }

    let mut fills: HashMap<Actor, Vec<Fill>> = HashMap::new();
    for (res, clearing) in clearings {
        let (bids, asks) = resource_books.get(res).unwrap();
        let filled_orders = bids[clearing.bid_idx..]
            .iter()
            .enumerate()
            .map(|(idx, x)| (idx + clearing.bid_idx, x))
            .chain(asks[..=clearing.ask_idx].iter().enumerate());

        for (idx, (order, _)) in filled_orders {
            fills.entry(order.actor).or_default().push(Fill {
                res,
                idx,
                net: order.qty * clearing.price * order.side.outflow_sign(),
            });
        }
    }

    println!("fills: {:?}\n", fills);

    let mut any_pruned = false;
    for (actor, actor_fills) in fills {
        let net_outflow: Fp = actor_fills.iter().map(|fill| fill.net).sum();
        let wallet = wallets[&actor];
        let balance = wallet - net_outflow;

        println!(
            "actor: {}, net_outflow: {}, balance: {}",
            actor, net_outflow, balance
        );

        if balance < fp(0) {
            any_pruned = true;
            let total_buys: Fp = actor_fills
                .iter()
                .map(|fill| {
                    if BidAsk::from_sign(fill.net) == Bid {
                        dbg!(fill.net)
                    } else {
                        fp(0)
                    }
                })
                .sum();
            println!("total_buys: {}", total_buys);

            // neg - bigger pos => pos
            let prune_numerator = dbg!((total_buys).abs()) - dbg!(balance.abs());
            assert!(prune_numerator >= fp(0));

            for fill in actor_fills {
                if fill.net > fp(0) {
                    let (bid, _) = &mut resource_books.get_mut(fill.res).unwrap().0[fill.idx];
                    bid.qty = dbg!(bid.qty) * prune_numerator / total_buys.abs();
                }
            }
        }
    }
    any_pruned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_net_outflows() {
        //
        let mut wallets: HashMap<u16, Fp> =
            HashMap::from_iter([(1, fp(0)), (2, fp(0)), (3, fp(0))]);
        let mut resource_books: HashMap<Resource, (Curve, Curve)> = HashMap::default();
        resource_books.insert(
            "wheat",
            (
                vec![(
                    Order {
                        actor: 1,
                        res: "wheat",
                        side: Bid,
                        limit: fp(6),
                        qty: fp(10),
                    },
                    fp(10),
                )],
                vec![
                    (
                        Order {
                            actor: 1,
                            res: "wheat",
                            side: Ask,
                            limit: fp(6),
                            qty: fp(5),
                        },
                        fp(5),
                    ),
                    (
                        Order {
                            actor: 2,
                            res: "wheat",
                            side: Ask,
                            limit: fp(6),
                            qty: fp(5),
                        },
                        fp(10),
                    ),
                ],
            ),
        );
        build_curves(&mut resource_books);
        let (bids, asks) = &resource_books["wheat"];
        let clearing = find_clearing_price(bids, asks).unwrap();

        let expected = Clearing {
            price: fp(6),
            matched_qty: fp(10),
            bid_idx: 0,
            ask_idx: 1,
        };
        assert_eq!(clearing, expected);

        let clearings = vec![("wheat", clearing)];

        net_outflows(&mut wallets, &mut resource_books, clearings.into_iter());
        println!("resource_books: {:?}", resource_books);
        assert_eq!(resource_books["wheat"].0[0].0.qty, fp(5));
    }

    fn mk_bid(limit: Fp, qty: Fp) -> (Order, Fp) {
        (
            Order {
                actor: 1,
                res: "wheat",
                side: Bid,
                limit,
                qty,
            },
            fp(0),
        )
    }

    fn mk_ask(limit: Fp, qty: Fp) -> (Order, Fp) {
        (
            Order {
                actor: 1,
                res: "wheat",
                side: Ask,
                limit,
                qty,
            },
            fp(0),
        )
    }

    #[test]
    fn test_find_clearing_price_no_overlap() {
        let (bids, asks) = setup_curve(
            vec![mk_bid(fp(5), fp(5)), mk_bid(fp(4), fp(3))],
            vec![mk_ask(fp(6), fp(4)), mk_ask(fp(7), fp(6))],
        );

        // Find clearing price
        let clearing = find_clearing_price(&bids, &asks);

        // Verify no clearing price is found since lowest ask (6) > highest bid (5)
        assert!(
            clearing.is_none(),
            "Expected no clearing price when bids and asks don't overlap"
        );
    }

    #[test]
    fn test_find_clearing_price_1_overlap() {
        let (bids, asks) = setup_curve(
            vec![mk_bid(fp(6), fp(5)), mk_bid(fp(4), fp(3))],
            vec![mk_ask(fp(5), fp(4)), mk_ask(fp(7), fp(6))],
        );

        // Find clearing price
        let clearing = find_clearing_price(&bids, &asks);

        // Verify no clearing price is found since lowest ask (6) > highest bid (5)
        assert_eq!(
            clearing,
            Some(Clearing {
                price: fp(5),
                matched_qty: fp(4),
                bid_idx: 1,
                ask_idx: 0,
            })
        );
    }

    #[test]
    fn test_find_clearing_price_multiple_bests() {
        let (bids, asks) = setup_curve(
            vec![mk_bid(fp(5), fp(2)), mk_bid(fp(4), fp(10))],
            vec![mk_ask(fp(4), fp(2)), mk_ask(fp(5), fp(10))],
        );

        // Find clearing price
        let clearing = find_clearing_price(&bids, &asks);

        // Verify no clearing price is found since lowest ask (6) > highest bid (5)
        assert_eq!(
            clearing,
            Some(Clearing {
                price: fp(4),
                matched_qty: fp(2),
                bid_idx: 0,
                ask_idx: 0,
            })
        );
    }

    fn setup_curve(bids: Vec<(Order, Fp)>, asks: Vec<(Order, Fp)>) -> (Curve, Curve) {
        let mut resource_books = HashMap::new();
        resource_books.insert("wheat", (bids, asks));
        build_curves(&mut resource_books);
        let (bids, asks) = resource_books.remove("wheat").unwrap();
        (bids, asks)
    }

    #[test]
    fn test_find_clearing_price_matches_bids_and_asks() {
        // Create bids with descending prices (highest first as sorted by build_curves)
        let bid1 = mk_bid(fp(10), fp(5)); // Highest price
        let bid2 = mk_bid(fp(8), fp(10));
        let bid3 = mk_bid(fp(6), fp(15)); // Lowest price

        // Create asks with ascending prices (lowest first as sorted by build_curves)
        let ask1 = mk_ask(fp(5), fp(8)); // Lowest price
        let ask2 = mk_ask(fp(7), fp(12));
        let ask3 = mk_ask(fp(9), fp(10)); // Highest price

        // Create and prepare the curves with cumulative quantities
        let (bids, asks) = setup_curve(vec![bid1, bid2, bid3], vec![ask1, ask2, ask3]);

        // Find the clearing price
        let clearing = find_clearing_price(&bids, &asks).unwrap();
        println!("clearing: {:?}", clearing);

        // Based on the function logic, the clearing price should be the ask price at the point
        // where the maximum quantity is matched
        // bid[1] (limit 8) and ask[1] (limit 7) should give the maximum matched quantity
        // This is because bid[1].cum = 15 and ask[1].cum = 20, giving a match of 35
        assert_eq!(clearing.price, fp(7)); // The price of the matched ask
        assert_eq!(clearing.matched_qty, fp(15)); // Sum of cumulative quantities
        assert_eq!(clearing.bid_idx, 1); // Index of the matched bid
        assert_eq!(clearing.ask_idx, 1); // Index of the matched ask
    }

    #[test]
    fn test_group_orders_multiple_actors() {
        let mut orders = HashMap::new();

        let actor1_order = Order {
            actor: 1,
            res: "wheat",
            side: Bid,
            limit: fp(7),
            qty: fp(10),
        };

        let actor2_order1 = Order {
            actor: 2,
            res: "wheat",
            side: Ask,
            limit: fp(8),
            qty: fp(5),
        };

        let actor2_order2 = Order {
            actor: 2,
            res: "wheat",
            side: Bid,
            limit: fp(6),
            qty: fp(3),
        };

        let actor3_order = Order {
            actor: 3,
            res: "corn",
            side: Ask,
            limit: fp(4),
            qty: fp(7),
        };

        orders.insert(1, vec![actor1_order]);
        orders.insert(2, vec![actor2_order1, actor2_order2]);
        orders.insert(3, vec![actor3_order]);

        let result = group_orders(&orders);

        assert_eq!(result.len(), 2);
        assert!(result.contains_key("wheat"));
        assert!(result.contains_key("corn"));

        let (wheat_bids, wheat_asks) = &result["wheat"];
        assert_eq!(wheat_bids.len(), 2);
        assert_eq!(wheat_asks.len(), 1);

        let (corn_bids, corn_asks) = &result["corn"];
        assert_eq!(corn_bids.len(), 0);
        assert_eq!(corn_asks.len(), 1);

        // Verify orders from specific actors are present
        let wheat_bid_actors: Vec<Actor> =
            wheat_bids.iter().map(|(order, _)| order.actor).collect();
        assert!(wheat_bid_actors.contains(&1));
        assert!(wheat_bid_actors.contains(&2));

        let wheat_ask_actors: Vec<Actor> =
            wheat_asks.iter().map(|(order, _)| order.actor).collect();
        assert!(wheat_ask_actors.contains(&2));

        let corn_ask_actors: Vec<Actor> = corn_asks.iter().map(|(order, _)| order.actor).collect();
        assert!(corn_ask_actors.contains(&3));
    }

    #[test]
    fn test_build_curves_sorts_orders_and_computes_cumulative_qty() {
        // Create a resource books HashMap with bids and asks for a single resource
        let mut resource_books = HashMap::new();

        // Create some orders with different prices for bids
        let bid1 = Order {
            actor: 1,
            res: "wheat",
            side: Bid,
            limit: fp(5), // Middle price
            qty: fp(10),
        };

        let bid2 = Order {
            actor: 2,
            res: "wheat",
            side: Bid,
            limit: fp(7), // Highest price
            qty: fp(15),
        };

        let bid3 = Order {
            actor: 3,
            res: "wheat",
            side: Bid,
            limit: fp(3), // Lowest price
            qty: fp(5),
        };

        // Create some orders with different prices for asks
        let ask1 = Order {
            actor: 4,
            res: "wheat",
            side: Ask,
            limit: fp(6), // Middle price
            qty: fp(8),
        };

        let ask2 = Order {
            actor: 5,
            res: "wheat",
            side: Ask,
            limit: fp(4), // Lowest price
            qty: fp(12),
        };

        let ask3 = Order {
            actor: 6,
            res: "wheat",
            side: Ask,
            limit: fp(9), // Highest price
            qty: fp(7),
        };

        // Create vectors for bids and asks with initial cumulative qty of 0
        let bids = vec![(bid1, fp(0)), (bid2, fp(0)), (bid3, fp(0))];

        let asks = vec![(ask1, fp(0)), (ask2, fp(0)), (ask3, fp(0))];

        // Insert into resource_books
        resource_books.insert("wheat", (bids, asks));

        // Call build_curves
        build_curves(&mut resource_books);

        // Get the updated bids and asks
        let (updated_bids, updated_asks) = &resource_books["wheat"];

        // Test that bids are sorted by limit price (descending)
        assert_eq!(updated_bids[0].0.limit, fp(3)); // Lowest price first
        assert_eq!(updated_bids[1].0.limit, fp(5));
        assert_eq!(updated_bids[2].0.limit, fp(7)); // Highest price last

        // Test that asks are sorted by limit price (ascending)
        assert_eq!(updated_asks[0].0.limit, fp(4)); // Lowest price first
        assert_eq!(updated_asks[1].0.limit, fp(6));
        assert_eq!(updated_asks[2].0.limit, fp(9)); // Highest price last

        // Test cumulative quantities for bids
        assert_eq!(updated_bids[0].1, fp(30)); // First order qty (highest price)
        assert_eq!(updated_bids[1].1, fp(25)); // First + second order qty
        assert_eq!(updated_bids[2].1, fp(15)); // First + second + third order qty

        // Test cumulative quantities for asks
        assert_eq!(updated_asks[0].1, fp(12)); // First order qty (lowest price)
        assert_eq!(updated_asks[1].1, fp(20)); // First + second order qty
        assert_eq!(updated_asks[2].1, fp(27)); // First + second + third order qty
    }

    #[test]
    fn test_build_curves_empty_orders() {
        // Test with empty bids and asks
        let mut resource_books = HashMap::new();
        let empty_bids: Vec<(Order, Qty)> = vec![];
        let empty_asks: Vec<(Order, Qty)> = vec![];

        resource_books.insert("wheat", (empty_bids, empty_asks));

        // This should not panic
        build_curves(&mut resource_books);

        let (updated_bids, updated_asks) = &resource_books["wheat"];
        assert!(updated_bids.is_empty());
        assert!(updated_asks.is_empty());
    }

    #[test]
    fn test_build_curves_multiple_resources() {
        // Create a resource books HashMap with bids and asks for multiple resources
        let mut resource_books = HashMap::new();

        // Wheat orders
        let wheat_bid = Order {
            actor: 1,
            res: "wheat",
            side: Bid,
            limit: fp(5),
            qty: fp(10),
        };

        let wheat_ask = Order {
            actor: 2,
            res: "wheat",
            side: Ask,
            limit: fp(6),
            qty: fp(8),
        };

        // Corn orders
        let corn_bid1 = Order {
            actor: 3,
            res: "corn",
            side: Bid,
            limit: fp(3),
            qty: fp(5),
        };

        let corn_bid2 = Order {
            actor: 4,
            res: "corn",
            side: Bid,
            limit: fp(4),
            qty: fp(7),
        };

        let corn_ask = Order {
            actor: 5,
            res: "corn",
            side: Ask,
            limit: fp(5),
            qty: fp(10),
        };

        // Create vectors for bids and asks with initial cumulative qty of 0
        let wheat_bids = vec![(wheat_bid, fp(0))];
        let wheat_asks = vec![(wheat_ask, fp(0))];

        let corn_bids = vec![(corn_bid1, fp(0)), (corn_bid2, fp(0))];
        let corn_asks = vec![(corn_ask, fp(0))];

        // Insert into resource_books
        resource_books.insert("wheat", (wheat_bids, wheat_asks));
        resource_books.insert("corn", (corn_bids, corn_asks));

        // Call build_curves
        build_curves(&mut resource_books);

        // Verify wheat orders
        let (wheat_updated_bids, wheat_updated_asks) = &resource_books["wheat"];
        assert_eq!(wheat_updated_bids[0].1, fp(10)); // Cumulative qty
        assert_eq!(wheat_updated_asks[0].1, fp(8)); // Cumulative qty

        // Verify corn orders are sorted and cumulative quantities computed
        let (corn_updated_bids, corn_updated_asks) = &resource_books["corn"];

        // Test that corn bids are sorted
        assert_eq!(corn_updated_bids[0].0.limit, fp(3)); // Higher price first
        assert_eq!(corn_updated_bids[1].0.limit, fp(4)); // Lower price last

        // Test cumulative quantities for corn bids
        assert_eq!(corn_updated_bids[0].1, fp(12)); // First + second order qty 
        assert_eq!(corn_updated_bids[1].1, fp(7)); // Second order qty

        // Test cumulative quantity for corn ask
        assert_eq!(corn_updated_asks[0].1, fp(10)); // Single ask qty
    }
}
