use terrazzo_synctex_sys as sys;

use crate::Node;
use crate::Result;
use crate::Scanner;
use crate::status_to_result;

/// Results returned by a SyncTeX query.
///
/// The results borrow the scanner that produced them. Iterating yields each
/// matching node in scanner order.
#[derive(Debug)]
pub struct QueryResults<'scanner> {
    scanner: &'scanner mut Scanner,
    count: usize,
}

impl<'scanner> QueryResults<'scanner> {
    pub(crate) fn new(
        scanner: &'scanner mut Scanner,
        status: sys::synctex_status_t,
    ) -> Result<Self> {
        let count = status_to_result(status)?;
        Ok(Self { scanner, count })
    }

    /// Returns the number of nodes reported by the query.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Returns `true` when the query did not report any matching nodes.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Resets the scanner result cursor to the first query result.
    pub fn reset(&mut self) -> Result<()> {
        status_to_result(unsafe { sys::synctex_scanner_reset_result(self.scanner.raw.as_ptr()) })
            .map(drop)
    }
}

impl<'scanner> Iterator for QueryResults<'scanner> {
    type Item = Node<'scanner>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe { Node::from_raw(sys::synctex_scanner_next_result(self.scanner.raw.as_ptr())) }
    }
}
