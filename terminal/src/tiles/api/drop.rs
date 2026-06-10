use super::state::STATE;
use super::Tile;

impl Drop for Tile {
    fn drop(&mut self) {
        fn aux(this: &mut Tile) -> Option<()> {
            let mut state = STATE.lock().unwrap();
            let state = state.as_mut()?;
            let drop_fns = state.remove(&this.id)?;
            for drop_fn in drop_fns {
                drop_fn(this.id)
            }
            Some(())
        }
        let _ = aux(self);
    }
}
