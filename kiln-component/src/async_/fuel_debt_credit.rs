//! Fuel debt and credit system for async task management
//!
//! This module implements a debt/credit system for fuel consumption across
//! async tasks, enabling fair scheduling and preventing fuel starvation.

use core::sync::atomic::{AtomicU64, Ordering};

use wrt_foundation::{Arc, CrateId, collections::StaticMap as BoundedMap, safe_managed_alloc};
use wrt_sync::Mutex;

use crate::ComponentInstanceId;
use crate::prelude::*;

// Import TaskId from the appropriate location
#[cfg(not(feature = "component-model-threading"))]
use super::fuel_async_executor::TaskId;
#[cfg(feature = "component-model-threading")]
use crate::threading::task_manager::TaskId;

/// Maximum debt that a task can accumulate
const MAX_TASK_DEBT: u64 = 10000;

/// Maximum credit that a task can accumulate
const MAX_TASK_CREDIT: u64 = 50000;

/// Default credit per task
const DEFAULT_CREDIT: u64 = 1000;

/// Fuel debt and credit management system
pub struct FuelDebtCreditSystem {
    /// Task debt balances
    task_debts: Arc<Mutex<BoundedMap<TaskId, u64, 256>>>,
    /// Task credit balances
    task_credits: Arc<Mutex<BoundedMap<TaskId, u64, 256>>>,
    /// Component credit balances
    component_credits: Arc<Mutex<BoundedMap<ComponentInstanceId, u64, 256>>>,
    /// Global debt counter
    global_debt: AtomicU64,
    /// Global credit counter
    global_credit: AtomicU64,
    /// Debt policy configuration
    debt_policy: DebtPolicy,
    /// Credit restriction policy
    credit_restriction: CreditRestriction,
}

/// Policy for managing task debt
#[derive(Debug, Clone, Copy)]
pub enum DebtPolicy {
    /// Never allow debt accumulation
    NeverAllow,
    /// Allow unlimited debt (dangerous)
    Unlimited,
    /// Strict debt limits - task blocked when exceeded
    Strict,
    /// Gradual debt forgiveness
    Forgiveness { rate: u64 },
    /// Limited debt with maximum cap
    LimitedDebt { max_debt: u64 },
    /// Moderate debt with interest
    ModerateDebt { max_debt: u64, interest_rate: f64 },
    /// Flexible debt with soft and hard limits
    FlexibleDebt {
        soft_limit: u64,
        hard_limit: u64,
        interest_rate: f64,
    },
}

/// Credit restriction policies
#[derive(Debug, Clone, Copy)]
pub enum CreditRestriction {
    /// No restrictions on credit accumulation
    None,
    /// Cap credits at maximum value
    Capped,
    /// Redistribute excess credits to other tasks
    Redistribute,
    /// Credit is scoped to a specific component
    ForComponent { component_id: ComponentInstanceId },
}

impl FuelDebtCreditSystem {
    /// Create new fuel debt/credit system
    pub fn new(debt_policy: DebtPolicy, credit_restriction: CreditRestriction) -> Result<Self> {
        let provider = safe_managed_alloc!(4096, CrateId::Component)?;

        Ok(Self {
            task_debts: Arc::new(Mutex::new(BoundedMap::new())),
            task_credits: Arc::new(Mutex::new(BoundedMap::new())),
            component_credits: Arc::new(Mutex::new(BoundedMap::new())),
            global_debt: AtomicU64::new(0),
            global_credit: AtomicU64::new(0),
            debt_policy,
            credit_restriction,
        })
    }

    /// Register a new task with default credit
    pub fn register_task(&self, task_id: TaskId) -> Result<()> {
        let mut credits = self.task_credits.lock();
        credits
            .insert(task_id, DEFAULT_CREDIT)
            .map_err(|_| Error::resource_limit_exceeded("Too many tasks in credit system"))?;

        let mut debts = self.task_debts.lock();
        debts
            .insert(task_id, 0)
            .map_err(|_| Error::resource_limit_exceeded("Too many tasks in debt system"))?;

        Ok(())
    }

    /// Consume fuel from task's credit/debt balance
    pub fn consume_fuel(&self, task_id: TaskId, fuel: u64) -> Result<bool> {
        let mut credits = self.task_credits.lock();
        let mut debts = self.task_debts.lock();

        let current_credit = credits.get(&task_id).copied().unwrap_or(0);
        let current_debt = debts.get(&task_id).copied().unwrap_or(0);

        if current_credit >= fuel {
            // Sufficient credit available
            credits.insert(task_id, current_credit - fuel).ok();
            Ok(true)
        } else {
            // Need to go into debt
            let debt_needed = fuel - current_credit;
            let new_debt = current_debt + debt_needed;

            match self.debt_policy {
                DebtPolicy::NeverAllow => {
                    Ok(false) // Never allow debt
                },
                DebtPolicy::Unlimited => {
                    credits.insert(task_id, 0).ok();
                    debts.insert(task_id, new_debt).ok();
                    self.global_debt.fetch_add(debt_needed, Ordering::Relaxed);
                    Ok(true)
                },
                DebtPolicy::Strict => {
                    if new_debt > MAX_TASK_DEBT {
                        Ok(false) // Reject fuel consumption
                    } else {
                        credits.insert(task_id, 0).ok();
                        debts.insert(task_id, new_debt).ok();
                        self.global_debt.fetch_add(debt_needed, Ordering::Relaxed);
                        Ok(true)
                    }
                },
                DebtPolicy::Forgiveness { rate: _ } => {
                    // Allow debt but track for future forgiveness
                    credits.insert(task_id, 0).ok();
                    debts.insert(task_id, new_debt).ok();
                    self.global_debt.fetch_add(debt_needed, Ordering::Relaxed);
                    Ok(true)
                },
                DebtPolicy::LimitedDebt { max_debt } => {
                    if new_debt > max_debt {
                        Ok(false)
                    } else {
                        credits.insert(task_id, 0).ok();
                        debts.insert(task_id, new_debt).ok();
                        self.global_debt.fetch_add(debt_needed, Ordering::Relaxed);
                        Ok(true)
                    }
                },
                DebtPolicy::ModerateDebt {
                    max_debt,
                    interest_rate: _,
                } => {
                    if new_debt > max_debt {
                        Ok(false)
                    } else {
                        credits.insert(task_id, 0).ok();
                        debts.insert(task_id, new_debt).ok();
                        self.global_debt.fetch_add(debt_needed, Ordering::Relaxed);
                        Ok(true)
                    }
                },
                DebtPolicy::FlexibleDebt {
                    soft_limit: _,
                    hard_limit,
                    interest_rate: _,
                } => {
                    if new_debt > hard_limit {
                        Ok(false)
                    } else {
                        credits.insert(task_id, 0).ok();
                        debts.insert(task_id, new_debt).ok();
                        self.global_debt.fetch_add(debt_needed, Ordering::Relaxed);
                        Ok(true)
                    }
                },
            }
        }
    }

    /// Add credit to a task
    pub fn add_credit(&self, task_id: TaskId, credit: u64) -> Result<()> {
        let mut credits = self.task_credits.lock();
        let current_credit = credits.get(&task_id).copied().unwrap_or(0);

        let new_credit = match self.credit_restriction {
            CreditRestriction::None => current_credit + credit,
            CreditRestriction::Capped => (current_credit + credit).min(MAX_TASK_CREDIT),
            CreditRestriction::Redistribute => {
                let capped_credit = (current_credit + credit).min(MAX_TASK_CREDIT);
                let excess = (current_credit + credit).saturating_sub(MAX_TASK_CREDIT);
                if excess > 0 {
                    // TODO: Redistribute excess to other tasks
                }
                capped_credit
            },
            CreditRestriction::ForComponent { .. } => {
                // For component-scoped credit, treat like capped
                (current_credit + credit).min(MAX_TASK_CREDIT)
            },
        };

        credits.insert(task_id, new_credit).ok();
        self.global_credit.fetch_add(credit, Ordering::Relaxed);
        Ok(())
    }

    /// Pay down debt for a task
    pub fn pay_debt(&self, task_id: TaskId, payment: u64) -> Result<u64> {
        let mut debts = self.task_debts.lock();
        let current_debt = debts.get(&task_id).copied().unwrap_or(0);

        if current_debt == 0 {
            return Ok(0); // No debt to pay
        }

        let actual_payment = payment.min(current_debt);
        let new_debt = current_debt - actual_payment;

        debts.insert(task_id, new_debt).ok();
        self.global_debt.fetch_sub(actual_payment, Ordering::Relaxed);

        Ok(actual_payment)
    }

    /// Get task's current debt
    pub fn get_task_debt(&self, task_id: TaskId) -> Result<u64> {
        let debts = self.task_debts.lock();
        Ok(debts.get(&task_id).copied().unwrap_or(0))
    }

    /// Get task's current credit
    pub fn get_task_credit(&self, task_id: TaskId) -> Result<u64> {
        let credits = self.task_credits.lock();
        Ok(credits.get(&task_id).copied().unwrap_or(0))
    }

    /// Check if task can consume specified fuel
    pub fn can_consume_fuel(&self, task_id: TaskId, fuel: u64) -> Result<bool> {
        let credits = self.task_credits.lock();
        let debts = self.task_debts.lock();

        let current_credit = credits.get(&task_id).copied().unwrap_or(0);
        let current_debt = debts.get(&task_id).copied().unwrap_or(0);

        if current_credit >= fuel {
            return Ok(true);
        }

        let debt_needed = fuel - current_credit;
        let new_debt = current_debt + debt_needed;

        match self.debt_policy {
            DebtPolicy::NeverAllow => Ok(false),
            DebtPolicy::Unlimited => Ok(true),
            DebtPolicy::Strict => Ok(new_debt <= MAX_TASK_DEBT),
            DebtPolicy::Forgiveness { rate: _ } => Ok(new_debt <= MAX_TASK_DEBT * 2),
            DebtPolicy::LimitedDebt { max_debt } => Ok(new_debt <= max_debt),
            DebtPolicy::ModerateDebt {
                max_debt,
                interest_rate: _,
            } => Ok(new_debt <= max_debt),
            DebtPolicy::FlexibleDebt {
                soft_limit: _,
                hard_limit,
                interest_rate: _,
            } => Ok(new_debt <= hard_limit),
        }
    }

    /// Process debt forgiveness (call periodically)
    pub fn process_debt_forgiveness(&self) -> Result<u64> {
        if let DebtPolicy::Forgiveness { rate } = self.debt_policy {
            let mut debts = self.task_debts.lock();
            let mut total_forgiven = 0u64;

            for (task_id, debt) in debts.iter_mut() {
                if *debt > 0 {
                    let forgiveness = (*debt).min(rate);
                    *debt -= forgiveness;
                    total_forgiven += forgiveness;
                }
            }

            self.global_debt.fetch_sub(total_forgiven, Ordering::Relaxed);
            Ok(total_forgiven)
        } else {
            Ok(0)
        }
    }

    /// Get global debt level
    pub fn global_debt(&self) -> u64 {
        self.global_debt.load(Ordering::Relaxed)
    }

    /// Get global credit level
    pub fn global_credit(&self) -> u64 {
        self.global_credit.load(Ordering::Relaxed)
    }

    /// Unregister a task (cleanup)
    pub fn unregister_task(&self, task_id: TaskId) -> Result<(u64, u64)> {
        let mut credits = self.task_credits.lock();
        let mut debts = self.task_debts.lock();

        let final_credit = credits.remove(&task_id).unwrap_or(0);
        let final_debt = debts.remove(&task_id).unwrap_or(0);

        // Update global counters
        self.global_credit.fetch_sub(final_credit, Ordering::Relaxed);
        self.global_debt.fetch_sub(final_debt, Ordering::Relaxed);

        Ok((final_credit, final_debt))
    }

    /// Grant credit to a component
    pub fn grant_credit(
        &mut self,
        component_id: ComponentInstanceId,
        amount: u64,
        restriction: CreditRestriction,
    ) -> Result<()> {
        let mut component_credits = self.component_credits.lock();
        let current_credit = component_credits.get(&component_id).copied().unwrap_or(0);

        let new_credit = match restriction {
            CreditRestriction::None => current_credit + amount,
            CreditRestriction::Capped | CreditRestriction::ForComponent { .. } => {
                (current_credit + amount).min(MAX_TASK_CREDIT)
            },
            CreditRestriction::Redistribute => {
                let capped_credit = (current_credit + amount).min(MAX_TASK_CREDIT);
                let excess = (current_credit + amount).saturating_sub(MAX_TASK_CREDIT);
                if excess > 0 {
                    // TODO: Redistribute excess to other components
                }
                capped_credit
            },
        };

        component_credits.insert(component_id, new_credit).ok();
        self.global_credit.fetch_add(amount, Ordering::Relaxed);
        Ok(())
    }

    /// Use credit from a component to cover a task's fuel deficit
    pub fn use_credit(
        &self,
        component_id: ComponentInstanceId,
        amount: u64,
        _task_id: TaskId,
    ) -> Result<u64> {
        let mut component_credits = self.component_credits.lock();
        let current_credit = component_credits.get(&component_id).copied().unwrap_or(0);

        if current_credit == 0 {
            return Ok(0); // No credit available
        }

        let credit_used = amount.min(current_credit);
        let new_credit = current_credit - credit_used;

        component_credits.insert(component_id, new_credit).ok();
        self.global_credit.fetch_sub(credit_used, Ordering::Relaxed);

        Ok(credit_used)
    }

    /// Get component's current credit balance
    pub fn get_component_credit(&self, component_id: ComponentInstanceId) -> u64 {
        let component_credits = self.component_credits.lock();
        component_credits.get(&component_id).copied().unwrap_or(0)
    }

    /// Check if a task can incur debt (policy-based decision)
    pub fn can_incur_debt(&self, task_id: TaskId, amount: u64, policy: &DebtPolicy) -> bool {
        let debts = self.task_debts.lock();
        let current_debt = debts.get(&task_id).copied().unwrap_or(0);
        let new_debt = current_debt + amount;

        match policy {
            DebtPolicy::NeverAllow => false,
            DebtPolicy::Unlimited => true,
            DebtPolicy::Strict => new_debt <= MAX_TASK_DEBT,
            DebtPolicy::Forgiveness { rate: _ } => new_debt <= MAX_TASK_DEBT * 2,
            DebtPolicy::LimitedDebt { max_debt } => new_debt <= *max_debt,
            DebtPolicy::ModerateDebt {
                max_debt,
                interest_rate: _,
            } => new_debt <= *max_debt,
            DebtPolicy::FlexibleDebt {
                soft_limit: _,
                hard_limit,
                interest_rate: _,
            } => new_debt <= *hard_limit,
        }
    }

    /// Incur debt for a task following the specified policy
    pub fn incur_debt(&mut self, task_id: TaskId, amount: u64, policy: &DebtPolicy) -> Result<()> {
        // Check if debt can be incurred under the policy
        if !self.can_incur_debt(task_id, amount, policy) {
            return Err(Error::async_fuel_exhausted("Debt limit exceeded for task"));
        }

        let mut debts = self.task_debts.lock();
        let current_debt = debts.get(&task_id).copied().unwrap_or(0);
        let new_debt = current_debt + amount;

        // Update task debt
        debts
            .insert(task_id, new_debt)
            .map_err(|_| Error::resource_limit_exceeded("Failed to update task debt"))?;

        // Update global debt counter
        self.global_debt.fetch_add(amount, Ordering::Relaxed);

        Ok(())
    }

    /// Repay debt for a task with interest applied
    pub fn repay_debt(&mut self, task_id: TaskId, payment: u64, interest_rate: f64) -> Result<u64> {
        let mut debts = self.task_debts.lock();
        let current_debt = debts.get(&task_id).copied().unwrap_or(0);

        if current_debt == 0 {
            return Ok(0); // No debt to repay
        }

        // Calculate interest on current debt
        let interest = (current_debt as f64 * interest_rate) as u64;
        let total_owed = current_debt.saturating_add(interest);

        // Calculate actual payment (capped at available payment)
        let actual_payment = payment.min(total_owed);
        let new_debt = total_owed.saturating_sub(actual_payment);

        // Update task debt
        debts
            .insert(task_id, new_debt)
            .map_err(|_| Error::resource_limit_exceeded("Failed to update task debt"))?;

        // Update global debt counter
        let debt_reduction = current_debt.saturating_sub(new_debt);
        self.global_debt.fetch_sub(debt_reduction, Ordering::Relaxed);

        Ok(actual_payment)
    }
}

impl Default for FuelDebtCreditSystem {
    fn default() -> Self {
        Self::new(DebtPolicy::Strict, CreditRestriction::Capped)
            .expect("Failed to create default FuelDebtCreditSystem")
    }
}
