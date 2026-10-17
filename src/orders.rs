//! Orders.

use crate::wire::wire_enum;

wire_enum! {
    /// The side of an order or a fill.
    OrderSide {
        /// Buy.
        Buy => "buy",
        /// Sell.
        Sell => "sell",
    }
}

wire_enum! {
    /// The status of an order.
    OrderStatus {
        /// The order has been received and routed; the usual initial state.
        New => "new",
        /// The order has been partially filled.
        PartiallyFilled => "partially_filled",
        /// The order has been filled; no further updates will occur.
        Filled => "filled",
        /// The order is done executing for the day.
        DoneForDay => "done_for_day",
        /// The order has been canceled; no further updates will occur.
        Canceled => "canceled",
        /// The order has expired; no further updates will occur.
        Expired => "expired",
        /// The order was replaced by another order.
        Replaced => "replaced",
        /// The order is waiting to be canceled.
        PendingCancel => "pending_cancel",
        /// The order is waiting to be replaced.
        PendingReplace => "pending_replace",
        /// The order has been received but not yet routed.
        Accepted => "accepted",
        /// The order has been routed but not yet accepted for execution.
        PendingNew => "pending_new",
        /// The order has been received and accepted for bidding.
        AcceptedForBidding => "accepted_for_bidding",
        /// The order has been stopped; a trade is guaranteed.
        Stopped => "stopped",
        /// The order has been rejected; no further updates will occur.
        Rejected => "rejected",
        /// The order has been suspended and cannot trade.
        Suspended => "suspended",
        /// The order is complete; settlement calculations are pending.
        Calculated => "calculated",
        /// The order is held.
        Held => "held",
    }
}
