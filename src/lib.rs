use anyhow::Context;
use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use solana_clock::{Epoch, Slot};
use solana_epoch_schedule::EpochSchedule;
use solana_pubkey::Pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use std::collections::BTreeMap;
use std::str::FromStr;

/// Fetches full leader schedule for the given epoch
async fn fetch_leader_schedule(
    epoch: u64,
    epoch_schedule: &EpochSchedule,
    rpc_client: &RpcClient,
) -> anyhow::Result<HashMap<Pubkey, Vec<Slot>>> {
    let epoch_offset = epoch_schedule.get_first_slot_in_epoch(epoch);

    rpc_client
        .get_leader_schedule(Some(epoch_offset))
        .await?
        .context("Failed to fetch leader schedule from RPC client")?
        .into_iter()
        .map(|(pubkey, slots)| {
            Ok((
                Pubkey::from_str(&pubkey).context("Failed to parse pubkey")?,
                slots
                    .into_iter()
                    .map(move |slot_index| slot_index as u64 + epoch_offset)
                    .sorted()
                    .collect::<Vec<_>>(),
            ))
        })
        .collect::<Result<HashMap<Pubkey, Vec<Slot>>, anyhow::Error>>()
}

pub struct LeaderSchedule {
    epoch: Epoch,
    epoch_schedule: EpochSchedule,
    leaders: HashMap<Pubkey, Vec<Slot>>,
    slots: Vec<Pubkey>,
}

fn build_schedule_by_slot(schedule: &HashMap<Pubkey, Vec<Slot>>) -> Vec<Pubkey> {
    let leaders_by_slot: BTreeMap<Slot, Pubkey> = schedule
        .iter()
        .flat_map(|(pubkey, slots)| slots.iter().map(move |slot| (*slot, *pubkey)))
        .collect();

    leaders_by_slot.into_values().collect()
}

impl LeaderSchedule {
    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    pub fn epoch_schedule(&self) -> &EpochSchedule {
        &self.epoch_schedule
    }

    pub async fn new(epoch: Epoch, rpc_client: &RpcClient) -> anyhow::Result<Self> {
        let epoch_schedule = rpc_client.get_epoch_schedule().await?;
        let leaders = fetch_leader_schedule(epoch, &epoch_schedule, rpc_client).await?;
        let slots = build_schedule_by_slot(&leaders);
        Ok(Self {
            epoch,
            epoch_schedule,
            leaders,
            slots,
        })
    }

    pub fn validator_set(&self, count: u64, stake: f64) -> HashSet<Pubkey> {
        let slots_per_epoch = self.epoch_schedule.slots_per_epoch as f64;

        let target_avg = stake / count as f64;

        let validators: Vec<(Pubkey, f64)> = self
            .leaders
            .iter()
            .map(|(pubkey, slots)| (*pubkey, slots.len() as f64 / slots_per_epoch))
            .sorted_by(|a, b| {
                let da = (a.1 - target_avg).abs();
                let db = (b.1 - target_avg).abs();
                da.total_cmp(&db)
            })
            .collect();

        let mut result = HashSet::new();
        let mut total_weight = 0.0;

        for (pubkey, weight) in &validators {
            if result.len() as u64 >= count || total_weight + weight > stake {
                break;
            }
            result.insert(*pubkey);
            total_weight += weight;
        }

        result
    }

    pub fn next_leader_and_slot_new(
        &self,
        from_slot: Slot,
        validators: &HashSet<Pubkey>,
    ) -> Option<(Pubkey, Slot)> {
        validators
            .iter()
            .filter_map(|key| {
                let (key, slots) = self.leaders.get_key_value(key)?;
                let candidate = slots.partition_point(|&x| x <= from_slot);
                let slot = slots.get(candidate)?;
                Some((key, *slot))
            })
            .min_by_key(|&(_, slot)| slot)
            .map(|(key, slot)| (*key, slot))
    }

    pub fn next_leader_and_slot_old(
        &self,
        from_slot: Slot,
        validators: &HashSet<Pubkey>,
    ) -> Option<(Pubkey, Slot)> {
        let (epoch, from_slot_idx) = self.epoch_schedule.get_epoch_and_slot_index(from_slot);
        if epoch != self.epoch {
            return None;
        }

        let start = from_slot_idx as usize + 1;

        self.slots[start..]
            .iter()
            .position(|key| validators.contains(key))
            .map(|idx| {
                let slot_idx = idx + start;
                let slot = self.epoch_schedule.get_first_slot_in_epoch(epoch) + slot_idx as u64;
                (self.slots[slot_idx], slot)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_fetch_validator() -> anyhow::Result<()> {
        dotenvy::dotenv().ok();
        let rpc_client = RpcClient::new(std::env::var("SOLANA_RPC_URL")?);
        let epoch = 960;
        let schedule = LeaderSchedule::new(epoch, &rpc_client).await?;
        println!("{} - {}", schedule.leaders.len(), schedule.slots.len());

        let validators = schedule.validator_set(50, 0.1);
        let stake = schedule
            .leaders
            .iter()
            .filter_map(|(pubkey, slots)| {
                if validators.contains(pubkey) {
                    Some(slots.len() as f64 / schedule.epoch_schedule.slots_per_epoch as f64)
                } else {
                    None
                }
            })
            .sum::<f64>();
        println!("{} - {} ", validators.len(), stake);

        let target_slot = schedule.epoch_schedule.get_first_slot_in_epoch(epoch) + schedule.epoch_schedule.get_slots_in_epoch(epoch) / 2;

        let old_algo = schedule.next_leader_and_slot_old(target_slot, &validators);
        let new_algo = schedule.next_leader_and_slot_new(target_slot, &validators);

        println!("old {:?}", old_algo);
        println!("new {:?}", new_algo);

        Ok(())
    }
}
