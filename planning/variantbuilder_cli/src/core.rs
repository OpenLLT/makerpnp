use std::sync::Arc;

use anyhow::anyhow;
use crossbeam_channel::Sender;
use tracing::trace;
use variantbuilder_app::{Effect, Event, VariantBuilder};

pub type Core = Arc<crux_core::Core<VariantBuilder>>;

pub fn new() -> Core {
    Arc::new(crux_core::Core::new())
}

pub fn update(core: &Core, event: Event, tx: &Arc<Sender<Effect>>) -> anyhow::Result<()> {
    trace!("event: {:?}", event);

    for effect in core.process_event(event) {
        process_effect(core, effect, tx)?;
    }
    Ok(())
}

pub fn process_effect(_core: &Core, effect: Effect, tx: &Arc<Sender<Effect>>) -> anyhow::Result<()> {
    trace!("effect: {:?}", effect);

    tx.send(effect)
        .map_err(|e| anyhow!("{:?}", e))?;

    Ok(())
}
