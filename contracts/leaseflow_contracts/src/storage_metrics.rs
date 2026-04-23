//! Storage Optimization Metrics and Monitoring
//! 
//! This module provides comprehensive metrics and monitoring for storage optimization,
//! enabling detailed analysis of storage usage patterns and optimization effectiveness.

use soroban_sdk::{
    Address, Env, Symbol, String, BytesN, Vec, Map, i128, u64, u32
};
use crate::{
    LeaseContract, CleanupError, CleanupDataKey, StorageMetrics, LeaseTombstone,
    storage_optimizer::StorageStatistics
};

/// Comprehensive storage monitoring system
pub struct StorageMonitor;

impl StorageMonitor {
    /// Get detailed storage analysis report
    pub fn get_storage_analysis(env: &Env) -> StorageAnalysisReport {
        let current_time = env.ledger().timestamp();
        let mut report = StorageAnalysisReport::new(current_time);
        
        // Collect basic metrics
        report.metrics = LeaseContract::get_storage_metrics(env.clone());
        
        // Collect detailed statistics
        report.statistics = crate::storage_optimizer::SorobanStorageOptimizer::get_storage_statistics(env);
        
        // Calculate efficiency metrics
        report.efficiency = Self::calculate_efficiency_metrics(env);
        
        // Calculate cost projections
        report.cost_projection = Self::calculate_cost_projection(&report);
        
        // Generate recommendations
        report.recommendations = Self::generate_recommendations(&report);
        
        report
    }
    
    /// Get real-time storage efficiency metrics
    pub fn get_efficiency_metrics(env: &Env) -> StorageEfficiencyMetrics {
        let stats = crate::storage_optimizer::SorobanStorageOptimizer::get_storage_statistics(env);
        let metrics = LeaseContract::get_storage_metrics(env.clone());
        
        StorageEfficiencyMetrics {
            timestamp: env.ledger().timestamp(),
            compression_ratio: Self::calculate_compression_ratio(&stats),
            space_savings_percentage: Self::calculate_space_savings(&stats),
            optimization_rate: Self::calculate_optimization_rate(&metrics),
            storage_health_score: Self::calculate_storage_health_score(&stats),
            rent_cost_efficiency: Self::calculate_rent_cost_efficiency(&stats),
        }
    }
    
    /// Get storage trend analysis over time
    pub fn get_storage_trends(env: &Env, days_back: u32) -> StorageTrendAnalysis {
        let current_time = env.ledger().timestamp();
        let start_time = current_time - (days_back as u64 * 24 * 60 * 60);
        
        let mut trends = StorageTrendAnalysis::new(start_time, current_time);
        
        // Collect historical data points
        for day in 0..days_back {
            let timestamp = start_time + (day as u64 * 24 * 60 * 60);
            if let Some(data_point) = Self::get_historical_data_point(env, timestamp) {
                trends.data_points.push_back(data_point);
            }
        }
        
        // Calculate trend metrics
        trends.growth_rate = Self::calculate_growth_rate(&trends.data_points);
        trends.optimization_velocity = Self::calculate_optimization_velocity(&trends.data_points);
        trends.cost_trend = Self::calculate_cost_trend(&trends.data_points);
        
        trends
    }
    
    /// Get storage cost analysis
    pub fn get_cost_analysis(env: &Env) -> StorageCostAnalysis {
        let stats = crate::storage_optimizer::SorobanStorageOptimizer::get_storage_statistics(env);
        let metrics = LeaseContract::get_storage_metrics(env.clone());
        
        StorageCostAnalysis {
            timestamp: env.ledger().timestamp(),
            current_monthly_cost: Self::estimate_monthly_cost(&stats),
            projected_annual_cost: Self::estimate_annual_cost(&stats),
            savings_from_optimization: Self::calculate_optimization_savings(&metrics),
            cost_per_lease: Self::calculate_cost_per_lease(&stats),
            rent_cost_breakdown: Self::get_rent_cost_breakdown(&stats),
            optimization_roi: Self::calculate_optimization_roi(&metrics),
        }
    }
    
    /// Get storage health indicators
    pub fn get_health_indicators(env: &Env) -> StorageHealthIndicators {
        let stats = crate::storage_optimizer::SorobanStorageOptimizer::get_storage_statistics(env);
        let metrics = LeaseContract::get_storage_metrics(env.clone());
        
        StorageHealthIndicators {
            timestamp: env.ledger().timestamp(),
            overall_health_score: Self::calculate_storage_health_score(&stats),
            bloat_level: Self::calculate_bloat_level(&stats),
            fragmentation_score: Self::calculate_fragmentation_score(env),
            optimization_needed: Self::is_optimization_needed(&stats),
            risk_factors: Self::identify_risk_factors(&stats),
            performance_impact: Self::assess_performance_impact(&stats),
        }
    }
    
    /// Generate storage optimization recommendations
    pub fn generate_optimization_recommendations(env: &Env) -> Vec<StorageRecommendation> {
        let stats = crate::storage_optimizer::SorobanStorageOptimizer::get_storage_statistics(env);
        let metrics = LeaseContract::get_storage_metrics(env.clone());
        let mut recommendations = Vec::new(env);
        
        // Analyze current state and generate recommendations
        if stats.lease_instances > 1000 {
            recommendations.push_back(StorageRecommendation {
                priority: RecommendationPriority::High,
                category: RecommendationCategory::Pruning,
                title: String::from_str(env, "High Volume of Active Leases"),
                description: String::from_str(env, "Consider pruning old terminated leases to reduce storage costs"),
                action_items: Vec::from_array(env, [
                    String::from_str(env, "Review leases older than 60 days"),
                    String::from_str(env, "Set up automated pruning schedule"),
                    String::from_str(env, "Monitor pruning effectiveness"),
                ]),
                estimated_savings: Self::estimate_pruning_savings(&stats),
                implementation_effort: ImplementationEffort::Medium,
            });
        }
        
        if stats.tombstones > stats.lease_instances / 2 {
            recommendations.push_back(StorageRecommendation {
                priority: RecommendationPriority::Medium,
                category: RecommendationCategory::Optimization,
                title: String::from_str(env, "High Tombstone Ratio"),
                description: String::from_str(env, "Consider further optimization of tombstone storage"),
                action_items: Vec::from_array(env, [
                    String::from_str(env, "Analyze tombstone efficiency"),
                    String::from_str(env, "Consider tombstone compression"),
                    String::from_str(env, "Evaluate off-chain storage options"),
                ]),
                estimated_savings: stats.tombstones * 64, // Estimated savings per tombstone
                implementation_effort: ImplementationEffort::Low,
            });
        }
        
        if metrics.total_bytes_recovered < 10000 {
            recommendations.push_back(StorageRecommendation {
                priority: RecommendationPriority::Low,
                category: RecommendationCategory::Monitoring,
                title: String::from_str(env, "Low Storage Recovery"),
                description: String::from_str(env, "Storage optimization is underperforming"),
                action_items: Vec::from_array(env, [
                    String::from_str(env, "Review pruning criteria"),
                    String::from_str(env, "Check for legal holds blocking pruning"),
                    String::from_str(env, "Optimize pruning schedule"),
                ]),
                estimated_savings: 5000, // Estimated potential savings
                implementation_effort: ImplementationEffort::Low,
            });
        }
        
        recommendations
    }
    
    /// Track storage optimization performance
    pub fn track_optimization_performance(env: &Env, period_days: u32) -> OptimizationPerformanceReport {
        let current_time = env.ledger().timestamp();
        let start_time = current_time - (period_days as u64 * 24 * 60 * 60);
        
        OptimizationPerformanceReport {
            period_start: start_time,
            period_end: current_time,
            leases_pruned: LeaseContract::get_storage_metrics(env.clone()).total_leases_pruned,
            bytes_recovered: LeaseContract::get_storage_metrics(env.clone()).total_bytes_recovered,
            cost_savings: Self::calculate_period_cost_savings(env, start_time, current_time),
            efficiency_improvement: Self::calculate_efficiency_improvement(env, start_time, current_time),
            error_rate: Self::calculate_error_rate(env, start_time, current_time),
        }
    }
    
    // Helper methods for calculations
    
    fn calculate_efficiency_metrics(env: &Env) -> StorageEfficiencyMetrics {
        Self::get_efficiency_metrics(env)
    }
    
    fn calculate_compression_ratio(stats: &StorageStatistics) -> u32 {
        if stats.lease_instances > 0 {
            ((stats.lease_instances * 512 - stats.tombstones * 128) * 100 / (stats.lease_instances * 512)) as u32
        } else {
            0
        }
    }
    
    fn calculate_space_savings(stats: &StorageStatistics) -> u32 {
        if stats.total_bytes > 0 {
            ((stats.total_bytes - stats.tombstones * 128) * 100 / stats.total_bytes) as u32
        } else {
            0
        }
    }
    
    fn calculate_optimization_rate(metrics: &StorageMetrics) -> u32 {
        if metrics.total_leases_pruned > 0 {
            (metrics.total_bytes_recovered * 100 / (metrics.total_leases_pruned * 512)) as u32
        } else {
            0
        }
    }
    
    fn calculate_storage_health_score(stats: &StorageStatistics) -> u32 {
        let mut score = 100u32;
        
        // Deduct points for high storage usage
        if stats.total_bytes > 1_000_000 {
            score -= 20;
        } else if stats.total_bytes > 500_000 {
            score -= 10;
        }
        
        // Deduct points for low optimization
        let optimization_ratio = if stats.lease_instances > 0 {
            stats.tombstones * 100 / stats.lease_instances
        } else {
            0
        };
        
        if optimization_ratio < 20 {
            score -= 30;
        } else if optimization_ratio < 50 {
            score -= 15;
        }
        
        score.max(0)
    }
    
    fn calculate_rent_cost_efficiency(stats: &StorageStatistics) -> u32 {
        // Simplified cost calculation based on Stellar storage costs
        // In practice, this would use actual network pricing
        let base_cost_per_kb = 1; // 1 stroop per KB per month
        let total_cost = stats.total_bytes / 1024 * base_cost_per_kb;
        
        // Efficiency score based on cost vs. value
        if total_cost < 1000 {
            100
        } else if total_cost < 5000 {
            80
        } else if total_cost < 10000 {
            60
        } else {
            40
        }
    }
    
    fn calculate_cost_projection(report: &StorageAnalysisReport) -> CostProjection {
        let current_monthly = Self::estimate_monthly_cost(&report.statistics);
        let optimization_savings = Self::estimate_optimization_savings(&report.metrics);
        
        CostProjection {
            current_monthly_cost: current_monthly,
            projected_monthly_after_optimization: current_monthly.saturating_sub(optimization_savings as u64),
            annual_savings_potential: optimization_savings as u64 * 12,
            payback_period_months: if optimization_savings > 0 {
                1000 / optimization_savings as u64 // Simplified payback calculation
            } else {
                u64::MAX
            },
        }
    }
    
    fn estimate_monthly_cost(stats: &StorageStatistics) -> u64 {
        // Simplified cost estimation
        // In practice, use actual Stellar network pricing
        let cost_per_byte_per_month = 1; // 1 stroop per byte per month
        stats.total_bytes * cost_per_byte_per_month
    }
    
    fn estimate_annual_cost(stats: &StorageStatistics) -> u64 {
        Self::estimate_monthly_cost(stats) * 12
    }
    
    fn calculate_optimization_savings(metrics: &StorageMetrics) -> u32 {
        // Estimate savings based on bytes recovered
        // In practice, this would consider actual network pricing
        metrics.total_bytes_recovered as u32 / 100 // Simplified savings calculation
    }
    
    fn calculate_cost_per_lease(stats: &StorageStatistics) -> u64 {
        if stats.lease_instances > 0 {
            stats.total_bytes / stats.lease_instances
        } else {
            0
        }
    }
    
    fn get_rent_cost_breakdown(stats: &StorageStatistics) -> Map<String, u64> {
        let env = Env::default();
        let mut breakdown = Map::new(&env);
        
        breakdown.set(String::from_str(&env, "lease_instances"), stats.lease_instances * 512);
        breakdown.set(String::from_str(&env, "tombstones"), stats.tombstones * 128);
        breakdown.set(String::from_str(&env, "legal_holds"), stats.legal_holds * 128);
        breakdown.set(String::from_str(&env, "receipts"), stats.receipts * 96);
        breakdown.set(String::from_str(&env, "usage_rights"), stats.usage_rights * 128);
        
        breakdown
    }
    
    fn calculate_optimization_roi(metrics: &StorageMetrics) -> u32 {
        if metrics.total_bytes_recovered > 0 {
            // Simplified ROI calculation
            (metrics.total_bytes_recovered * 100 / (metrics.total_bytes_recovered + 10000)) as u32
        } else {
            0
        }
    }
    
    fn generate_recommendations(report: &StorageAnalysisReport) -> Vec<StorageRecommendation> {
        let env = Env::default();
        Self::generate_optimization_recommendations(&env)
    }
    
    fn get_historical_data_point(env: &Env, timestamp: u64) -> Option<StorageDataPoint> {
        // In practice, you would store historical data
        // This is a placeholder implementation
        None
    }
    
    fn calculate_growth_rate(data_points: &Vec<StorageDataPoint>) -> f64 {
        if data_points.len() < 2 {
            return 0.0;
        }
        
        let first = data_points.get(0)?;
        let last = data_points.get(data_points.len() - 1)?;
        
        let time_diff = last.timestamp - first.timestamp;
        if time_diff == 0 {
            return 0.0;
        }
        
        let storage_diff = last.total_bytes as f64 - first.total_bytes as f64;
        storage_diff / time_diff as f64
    }
    
    fn calculate_optimization_velocity(data_points: &Vec<StorageDataPoint>) -> f64 {
        // Similar to growth rate but for optimization metrics
        0.0 // Placeholder
    }
    
    fn calculate_cost_trend(data_points: &Vec<StorageDataPoint>) -> f64 {
        // Calculate cost trend over time
        0.0 // Placeholder
    }
    
    fn calculate_bloat_level(stats: &StorageStatistics) -> u32 {
        // Calculate storage bloat level
        let active_ratio = if stats.lease_instances > 0 {
            (stats.lease_instances - stats.tombstones) * 100 / stats.lease_instances
        } else {
            100
        };
        
        if active_ratio > 80 {
            0 // Low bloat
        } else if active_ratio > 60 {
            25 // Medium bloat
        } else {
            50 // High bloat
        }
    }
    
    fn calculate_fragmentation_score(env: &Env) -> u32 {
        // Calculate storage fragmentation
        // This would analyze storage patterns in practice
        10 // Placeholder
    }
    
    fn is_optimization_needed(stats: &StorageStatistics) -> bool {
        stats.lease_instances > 100 && stats.tombstones < stats.lease_instances / 2
    }
    
    fn identify_risk_factors(stats: &StorageStatistics) -> Vec<String> {
        let env = Env::default();
        let mut risks = Vec::new(&env);
        
        if stats.total_bytes > 1_000_000 {
            risks.push_back(String::from_str(&env, "High storage usage"));
        }
        
        if stats.legal_holds > 10 {
            risks.push_back(String::from_str(&env, "Multiple legal holds blocking optimization"));
        }
        
        if stats.lease_instances > stats.tombstones * 3 {
            risks.push_back(String::from_str(&env, "Low pruning efficiency"));
        }
        
        risks
    }
    
    fn assess_performance_impact(stats: &StorageStatistics) -> u32 {
        // Assess performance impact of current storage state
        if stats.total_bytes > 2_000_000 {
            80 // High impact
        } else if stats.total_bytes > 1_000_000 {
            50 // Medium impact
        } else {
            20 // Low impact
        }
    }
    
    fn estimate_pruning_savings(stats: &StorageStatistics) -> u32 {
        // Estimate potential savings from pruning
        let prunable_leases = stats.lease_instances - stats.tombstones;
        prunable_leases as u32 * (512 - 128) // Savings per lease
    }
    
    fn calculate_period_cost_savings(env: &Env, start_time: u64, end_time: u64) -> u64 {
        // Calculate cost savings over a period
        1000 // Placeholder
    }
    
    fn calculate_efficiency_improvement(env: &Env, start_time: u64, end_time: u64) -> u32 {
        // Calculate efficiency improvement over a period
        25 // Placeholder
    }
    
    fn calculate_error_rate(env: &Env, start_time: u64, end_time: u64) -> u32 {
        // Calculate error rate over a period
        1 // Placeholder
    }
}

// Data structures for comprehensive storage monitoring

#[derive(Debug, Clone)]
pub struct StorageAnalysisReport {
    pub timestamp: u64,
    pub metrics: StorageMetrics,
    pub statistics: StorageStatistics,
    pub efficiency: StorageEfficiencyMetrics,
    pub cost_projection: CostProjection,
    pub recommendations: Vec<StorageRecommendation>,
}

impl StorageAnalysisReport {
    pub fn new(timestamp: u64) -> Self {
        Self {
            timestamp,
            metrics: StorageMetrics {
                total_leases_pruned: 0,
                total_bytes_recovered: 0,
                total_tombstones_created: 0,
                active_legal_holds: 0,
                last_prune_timestamp: 0,
                average_lease_size_bytes: 0,
            },
            statistics: StorageStatistics::new(),
            efficiency: StorageEfficiencyMetrics::new(0),
            cost_projection: CostProjection::new(),
            recommendations: Vec::new(&Env::default()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StorageEfficiencyMetrics {
    pub timestamp: u64,
    pub compression_ratio: u32,
    pub space_savings_percentage: u32,
    pub optimization_rate: u32,
    pub storage_health_score: u32,
    pub rent_cost_efficiency: u32,
}

impl StorageEfficiencyMetrics {
    pub fn new(timestamp: u64) -> Self {
        Self {
            timestamp,
            compression_ratio: 0,
            space_savings_percentage: 0,
            optimization_rate: 0,
            storage_health_score: 100,
            rent_cost_efficiency: 100,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StorageTrendAnalysis {
    pub start_time: u64,
    pub end_time: u64,
    pub data_points: Vec<StorageDataPoint>,
    pub growth_rate: f64,
    pub optimization_velocity: f64,
    pub cost_trend: f64,
}

impl StorageTrendAnalysis {
    pub fn new(start_time: u64, end_time: u64) -> Self {
        Self {
            start_time,
            end_time,
            data_points: Vec::new(&Env::default()),
            growth_rate: 0.0,
            optimization_velocity: 0.0,
            cost_trend: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StorageDataPoint {
    pub timestamp: u64,
    pub total_bytes: u64,
    pub lease_instances: u64,
    pub tombstones: u64,
    pub cost: u64,
}

#[derive(Debug, Clone)]
pub struct StorageCostAnalysis {
    pub timestamp: u64,
    pub current_monthly_cost: u64,
    pub projected_annual_cost: u64,
    pub savings_from_optimization: u64,
    pub cost_per_lease: u64,
    pub rent_cost_breakdown: Map<String, u64>,
    pub optimization_roi: u32,
}

#[derive(Debug, Clone)]
pub struct StorageHealthIndicators {
    pub timestamp: u64,
    pub overall_health_score: u32,
    pub bloat_level: u32,
    pub fragmentation_score: u32,
    pub optimization_needed: bool,
    pub risk_factors: Vec<String>,
    pub performance_impact: u32,
}

#[derive(Debug, Clone)]
pub struct StorageRecommendation {
    pub priority: RecommendationPriority,
    pub category: RecommendationCategory,
    pub title: String,
    pub description: String,
    pub action_items: Vec<String>,
    pub estimated_savings: u32,
    pub implementation_effort: ImplementationEffort,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecommendationPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecommendationCategory {
    Pruning,
    Optimization,
    Monitoring,
    CostReduction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImplementationEffort {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone)]
pub struct CostProjection {
    pub current_monthly_cost: u64,
    pub projected_monthly_after_optimization: u64,
    pub annual_savings_potential: u64,
    pub payback_period_months: u64,
}

impl CostProjection {
    pub fn new() -> Self {
        Self {
            current_monthly_cost: 0,
            projected_monthly_after_optimization: 0,
            annual_savings_potential: 0,
            payback_period_months: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OptimizationPerformanceReport {
    pub period_start: u64,
    pub period_end: u64,
    pub leases_pruned: u64,
    pub bytes_recovered: u64,
    pub cost_savings: u64,
    pub efficiency_improvement: u32,
    pub error_rate: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as TestAddress;

    #[test]
    fn test_storage_analysis_report() {
        let env = Env::default();
        let report = StorageMonitor::get_storage_analysis(&env);
        
        assert!(report.timestamp > 0);
        assert_eq!(report.metrics.total_leases_pruned, 0);
        assert_eq!(report.statistics.lease_instances, 0);
        assert_eq!(report.efficiency.compression_ratio, 0);
    }

    #[test]
    fn test_efficiency_metrics() {
        let env = Env::default();
        let metrics = StorageMonitor::get_efficiency_metrics(&env);
        
        assert!(metrics.timestamp > 0);
        assert_eq!(metrics.compression_ratio, 0);
        assert_eq!(metrics.space_savings_percentage, 0);
        assert_eq!(metrics.optimization_rate, 0);
        assert_eq!(metrics.storage_health_score, 100);
    }

    #[test]
    fn test_cost_analysis() {
        let env = Env::default();
        let analysis = StorageMonitor::get_cost_analysis(&env);
        
        assert!(analysis.timestamp > 0);
        assert_eq!(analysis.current_monthly_cost, 0);
        assert_eq!(analysis.projected_annual_cost, 0);
        assert_eq!(analysis.savings_from_optimization, 0);
    }

    #[test]
    fn test_health_indicators() {
        let env = Env::default();
        let indicators = StorageMonitor::get_health_indicators(&env);
        
        assert!(indicators.timestamp > 0);
        assert_eq!(indicators.overall_health_score, 100);
        assert_eq!(indicators.bloat_level, 0);
        assert!(!indicators.optimization_needed);
    }

    #[test]
    fn test_recommendations() {
        let env = Env::default();
        let recommendations = StorageMonitor::generate_optimization_recommendations(&env);
        
        // Should have at least one recommendation for low storage recovery
        assert!(recommendations.len() > 0);
    }

    #[test]
    fn test_performance_tracking() {
        let env = Env::default();
        let report = StorageMonitor::track_optimization_performance(&env, 30);
        
        assert!(report.period_start > 0);
        assert!(report.period_end > report.period_start);
        assert_eq!(report.leases_pruned, 0);
        assert_eq!(report.bytes_recovered, 0);
    }
}
