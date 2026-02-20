//! Diagnostic caching and incremental updates
//!
//! This module provides caching functionality for diagnostic results to improve
//! performance on large codebases by avoiding re-analysis of unchanged files.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};

use crate::{
    diagnostics::{Diagnostic, DiagnosticCollection},
    error::{BuildError, BuildResult},
};

/// Cache metadata for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCacheEntry {
    /// File path relative to workspace
    pub file_path: PathBuf,
    /// Last modification time of the file when cached
    pub last_modified: SystemTime,
    /// File hash for additional verification
    pub file_hash: String,
    /// Cached diagnostics for this file
    pub diagnostics: Vec<Diagnostic>,
    /// When this cache entry was created
    pub cached_at: SystemTime,
}

/// Diagnostic cache for workspace files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticCache {
    /// Cache format version for compatibility
    pub version: u32,
    /// Workspace root path
    pub workspace_root: PathBuf,
    /// Cache entries by file path
    pub entries: HashMap<PathBuf, FileCacheEntry>,
    /// Global cache metadata
    pub metadata: CacheMetadata,
}

/// Cache metadata and statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetadata {
    /// When the cache was created
    pub created_at: SystemTime,
    /// When the cache was last updated
    pub updated_at: SystemTime,
    /// Total number of cached files
    pub total_files: usize,
    /// Cache hit statistics
    pub stats: CacheStats,
}

/// Cache usage statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: usize,
    /// Number of cache misses
    pub misses: usize,
    /// Number of files invalidated due to changes
    pub invalidations: usize,
    /// Total diagnostics cached
    pub total_diagnostics: usize,
}

/// Result of a cache lookup operation
#[derive(Debug)]
pub enum CacheResult {
    /// Cache hit - diagnostics found and valid
    Hit(Vec<Diagnostic>),
    /// Cache miss - file not in cache or cache invalid
    Miss,
    /// File changed - cached entry exists but file was modified
    Changed,
}

/// Incremental update result
#[derive(Debug)]
pub struct IncrementalUpdate {
    /// Files that were processed (changed or new)
    pub processed_files: Vec<PathBuf>,
    /// Files that were cached (unchanged)
    pub cached_files: Vec<PathBuf>,
    /// New diagnostics from processed files
    pub new_diagnostics: Vec<Diagnostic>,
    /// Total diagnostics (new + cached)
    pub all_diagnostics: Vec<Diagnostic>,
    /// Update statistics
    pub stats: UpdateStats,
}

/// Statistics for an incremental update
#[derive(Debug, Default)]
pub struct UpdateStats {
    /// Files processed from scratch
    pub files_processed: usize,
    /// Files served from cache
    pub files_cached: usize,
    /// Time saved by caching (estimated)
    pub time_saved_ms: u64,
}

impl DiagnosticCache {
    /// Cache format version
    const VERSION: u32 = 1;

    /// Create a new empty cache
    pub fn new(workspace_root: PathBuf) -> Self {
        let now = SystemTime::now();
        Self {
            version: Self::VERSION,
            workspace_root,
            entries: HashMap::new(),
            metadata: CacheMetadata {
                created_at: now,
                updated_at: now,
                total_files: 0,
                stats: CacheStats::default(),
            },
        }
    }

    /// Load cache from disk
    pub fn load<P: AsRef<Path>>(cache_path: P, workspace_root: PathBuf) -> BuildResult<Self> {
        let cache_path = cache_path.as_ref();

        if !cache_path.exists() {
            return Ok(Self::new(workspace_root));
        }

        let cache_content = fs::read_to_string(cache_path)
            .map_err(|e| BuildError::Tool(format!("Failed to read cache file: {}", e)))?;

        let mut cache: DiagnosticCache = serde_json::from_str(&cache_content)
            .map_err(|e| BuildError::Tool(format!("Failed to parse cache file: {}", e)))?;

        // Verify cache version compatibility
        if cache.version != Self::VERSION {
            return Ok(Self::new(workspace_root));
        }

        // Update workspace root if changed
        cache.workspace_root = workspace_root;

        Ok(cache)
    }

    /// Save cache to disk
    pub fn save<P: AsRef<Path>>(&self, cache_path: P) -> BuildResult<()> {
        let cache_path = cache_path.as_ref();

        // Ensure cache directory exists
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                BuildError::Tool(format!("Failed to create cache directory: {}", e))
            })?;
        }

        let cache_content = serde_json::to_string_pretty(self)
            .map_err(|e| BuildError::Tool(format!("Failed to serialize cache: {}", e)))?;

        fs::write(cache_path, cache_content)
            .map_err(|e| BuildError::Tool(format!("Failed to write cache file: {}", e)))?;

        Ok(())
    }

    /// Check if a file is cached and up-to-date
    pub fn check_file(&mut self, file_path: &Path) -> BuildResult<CacheResult> {
        let relative_path = self.make_relative_path(file_path);

        // Check if file exists in cache
        if let Some(entry) = self.entries.get(&relative_path) {
            // Check if file still exists
            if !file_path.exists() {
                self.entries.remove(&relative_path);
                self.metadata.stats.invalidations += 1;
                return Ok(CacheResult::Miss);
            }

            // Check modification time
            let current_modified = file_path
                .metadata()
                .and_then(|m| m.modified())
                .map_err(|e| BuildError::Tool(format!("Failed to get file metadata: {}", e)))?;

            if current_modified > entry.last_modified {
                self.metadata.stats.invalidations += 1;
                return Ok(CacheResult::Changed);
            }

            // Optionally verify file hash for additional safety
            let current_hash = self.calculate_file_hash(file_path)?;
            if current_hash != entry.file_hash {
                self.metadata.stats.invalidations += 1;
                return Ok(CacheResult::Changed);
            }

            // Cache hit
            self.metadata.stats.hits += 1;
            return Ok(CacheResult::Hit(entry.diagnostics.clone()));
        }

        // Cache miss
        self.metadata.stats.misses += 1;
        Ok(CacheResult::Miss)
    }

    /// Cache diagnostics for a file
    pub fn cache_file(
        &mut self,
        file_path: &Path,
        diagnostics: Vec<Diagnostic>,
    ) -> BuildResult<()> {
        let relative_path = self.make_relative_path(file_path);

        if !file_path.exists() {
            return Err(BuildError::Tool(format!(
                "File does not exist: {}",
                file_path.display()
            )));
        }

        let last_modified = file_path
            .metadata()
            .and_then(|m| m.modified())
            .map_err(|e| BuildError::Tool(format!("Failed to get file metadata: {}", e)))?;

        let file_hash = self.calculate_file_hash(file_path)?;

        let entry = FileCacheEntry {
            file_path: relative_path.clone(),
            last_modified,
            file_hash,
            diagnostics,
            cached_at: SystemTime::now(),
        };

        self.entries.insert(relative_path, entry);
        self.metadata.updated_at = SystemTime::now();
        self.metadata.total_files = self.entries.len();

        // Update diagnostic count
        self.metadata.stats.total_diagnostics =
            self.entries.values().map(|e| e.diagnostics.len()).sum();

        Ok(())
    }

    /// Remove file from cache
    pub fn invalidate_file(&mut self, file_path: &Path) {
        let relative_path = self.make_relative_path(file_path);
        if self.entries.remove(&relative_path).is_some() {
            self.metadata.stats.invalidations += 1;
            self.metadata.total_files = self.entries.len();
            self.metadata.updated_at = SystemTime::now();
        }
    }

    /// Clear entire cache
    pub fn clear(&mut self) {
        self.entries.clear();
        self.metadata.total_files = 0;
        self.metadata.stats = CacheStats::default();
        self.metadata.updated_at = SystemTime::now();
    }

    /// Get cache statistics
    pub fn stats(&self) -> &CacheStats {
        &self.metadata.stats
    }

    /// Calculate file hash for verification
    fn calculate_file_hash(&self, file_path: &Path) -> BuildResult<String> {
        let content = fs::read(file_path)
            .map_err(|e| BuildError::Tool(format!("Failed to read file for hashing: {}", e)))?;

        let hash = md5::compute(&content);
        Ok(format!("{:x}", hash))
    }

    /// Convert absolute path to relative path
    fn make_relative_path(&self, file_path: &Path) -> PathBuf {
        file_path.strip_prefix(&self.workspace_root).unwrap_or(file_path).to_path_buf()
    }

    /// Get all cached files
    pub fn cached_files(&self) -> Vec<&PathBuf> {
        self.entries.keys().collect()
    }

    /// Get cache size in bytes (estimated)
    pub fn estimated_size_bytes(&self) -> usize {
        // Rough estimation based on serialized size
        serde_json::to_string(self).map(|s| s.len()).unwrap_or(0)
    }
}

/// Cache manager for handling cache operations
pub struct CacheManager {
    cache: DiagnosticCache,
    cache_path: PathBuf,
    enabled: bool,
}

impl CacheManager {
    /// Create a new cache manager
    pub fn new(workspace_root: PathBuf, cache_path: PathBuf, enabled: bool) -> BuildResult<Self> {
        let cache = if enabled {
            DiagnosticCache::load(&cache_path, workspace_root.clone())?
        } else {
            DiagnosticCache::new(workspace_root)
        };

        Ok(Self {
            cache,
            cache_path,
            enabled,
        })
    }

    /// Check if caching is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable caching
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Get cached diagnostics for a file if available and valid
    pub fn get_cached(&mut self, file_path: &Path) -> BuildResult<Option<Vec<Diagnostic>>> {
        if !self.enabled {
            return Ok(None);
        }

        match self.cache.check_file(file_path)? {
            CacheResult::Hit(diagnostics) => Ok(Some(diagnostics)),
            CacheResult::Miss | CacheResult::Changed => Ok(None),
        }
    }

    /// Cache diagnostics for a file
    pub fn cache_diagnostics(
        &mut self,
        file_path: &Path,
        diagnostics: Vec<Diagnostic>,
    ) -> BuildResult<()> {
        if !self.enabled {
            return Ok(());
        }

        self.cache.cache_file(file_path, diagnostics)
    }

    /// Save cache to disk
    pub fn save(&self) -> BuildResult<()> {
        if !self.enabled {
            return Ok(());
        }

        self.cache.save(&self.cache_path)
    }

    /// Get cache statistics
    pub fn stats(&self) -> &CacheStats {
        self.cache.stats()
    }

    /// Clear cache
    pub fn clear(&mut self) -> BuildResult<()> {
        self.cache.clear();
        if self.enabled {
            self.save()?;
        }
        Ok(())
    }

    /// Get cache info for display
    pub fn info(&self) -> CacheInfo {
        CacheInfo {
            enabled: self.enabled,
            total_files: self.cache.metadata.total_files,
            total_diagnostics: self.cache.metadata.stats.total_diagnostics,
            cache_hits: self.cache.metadata.stats.hits,
            cache_misses: self.cache.metadata.stats.misses,
            invalidations: self.cache.metadata.stats.invalidations,
            estimated_size_bytes: self.cache.estimated_size_bytes(),
            created_at: self.cache.metadata.created_at,
            updated_at: self.cache.metadata.updated_at,
        }
    }

    /// Compare current diagnostics with cached ones and return diff
    pub fn compute_diff(&self, current_diagnostics: &[Diagnostic]) -> DiagnosticDiff {
        if !self.enabled {
            // If caching is disabled, all diagnostics are "new"
            return DiagnosticDiff {
                new_diagnostics: current_diagnostics.to_vec(),
                removed_diagnostics: Vec::new(),
                changed_diagnostics: Vec::new(),
                unchanged_diagnostics: Vec::new(),
                summary: DiffSummary {
                    new_count: current_diagnostics.len(),
                    removed_count: 0,
                    changed_count: 0,
                    unchanged_count: 0,
                },
            };
        }

        let mut new_diagnostics = Vec::new();
        let mut removed_diagnostics = Vec::new();
        let mut changed_diagnostics = Vec::new();
        let mut unchanged_diagnostics = Vec::new();

        // Collect all cached diagnostics
        let mut cached_diagnostics = Vec::new();
        for entry in self.cache.entries.values() {
            cached_diagnostics.extend(entry.diagnostics.iter().cloned());
        }

        // Create maps for efficient lookup
        let mut cached_map = std::collections::HashMap::new();
        for diagnostic in &cached_diagnostics {
            let key = diagnostic_key(diagnostic);
            cached_map.insert(key, diagnostic);
        }

        let mut current_map = std::collections::HashMap::new();
        for diagnostic in current_diagnostics {
            let key = diagnostic_key(diagnostic);
            current_map.insert(key, diagnostic);
        }

        // Find new and changed diagnostics
        for diagnostic in current_diagnostics {
            let key = diagnostic_key(diagnostic);
            if let Some(cached_diagnostic) = cached_map.get(&key) {
                if diagnostics_equal(diagnostic, cached_diagnostic) {
                    unchanged_diagnostics.push(diagnostic.clone());
                } else {
                    changed_diagnostics.push(((*cached_diagnostic).clone(), diagnostic.clone()));
                }
            } else {
                new_diagnostics.push(diagnostic.clone());
            }
        }

        // Find removed diagnostics
        for diagnostic in &cached_diagnostics {
            let key = diagnostic_key(diagnostic);
            if !current_map.contains_key(&key) {
                removed_diagnostics.push(diagnostic.clone());
            }
        }

        DiagnosticDiff {
            new_diagnostics: new_diagnostics.clone(),
            removed_diagnostics: removed_diagnostics.clone(),
            changed_diagnostics: changed_diagnostics.clone(),
            unchanged_diagnostics: unchanged_diagnostics.clone(),
            summary: DiffSummary {
                new_count: new_diagnostics.len(),
                removed_count: removed_diagnostics.len(),
                changed_count: changed_diagnostics.len(),
                unchanged_count: unchanged_diagnostics.len(),
            },
        }
    }

    /// Get only new and changed diagnostics (for diff mode)
    pub fn get_diff_diagnostics(&self, current_diagnostics: &[Diagnostic]) -> Vec<Diagnostic> {
        let diff = self.compute_diff(current_diagnostics);
        let mut result = diff.new_diagnostics;

        // Add changed diagnostics (new versions only)
        for (_, new_diagnostic) in diff.changed_diagnostics {
            result.push(new_diagnostic);
        }

        result
    }
}

/// Cache information for display
#[derive(Debug)]
pub struct CacheInfo {
    /// Whether caching is enabled
    pub enabled: bool,
    /// Total number of files in cache
    pub total_files: usize,
    /// Total number of diagnostics in cache
    pub total_diagnostics: usize,
    /// Number of cache hits
    pub cache_hits: usize,
    /// Number of cache misses
    pub cache_misses: usize,
    /// Number of cache invalidations
    pub invalidations: usize,
    /// Estimated cache size in bytes
    pub estimated_size_bytes: usize,
    /// When cache was created
    pub created_at: SystemTime,
    /// When cache was last updated
    pub updated_at: SystemTime,
}

/// Diagnostic diff result showing changes
#[derive(Debug, Clone)]
pub struct DiagnosticDiff {
    /// Diagnostics that are new (not in previous cache)
    pub new_diagnostics: Vec<Diagnostic>,
    /// Diagnostics that were removed (in previous cache but not current)
    pub removed_diagnostics: Vec<Diagnostic>,
    /// Diagnostics that changed between cache and current
    pub changed_diagnostics: Vec<(Diagnostic, Diagnostic)>, // (old, new)
    /// Diagnostics that remained the same
    pub unchanged_diagnostics: Vec<Diagnostic>,
    /// Summary of changes
    pub summary: DiffSummary,
}

/// Summary of diagnostic changes
#[derive(Debug, Clone, Default)]
pub struct DiffSummary {
    /// Number of new diagnostics
    pub new_count: usize,
    /// Number of removed diagnostics
    pub removed_count: usize,
    /// Number of changed diagnostics
    pub changed_count: usize,
    /// Number of unchanged diagnostics
    pub unchanged_count: usize,
}

/// Create a unique key for a diagnostic for comparison
fn diagnostic_key(diagnostic: &Diagnostic) -> String {
    format!(
        "{}:{}:{}:{}",
        diagnostic.file,
        diagnostic.range.start.line,
        diagnostic.range.start.character,
        diagnostic.severity as u8
    )
}

/// Check if two diagnostics are equal for diff purposes
fn diagnostics_equal(a: &Diagnostic, b: &Diagnostic) -> bool {
    a.file == b.file
        && a.range == b.range
        && a.severity == b.severity
        && a.code == b.code
        && a.message == b.message
        && a.source == b.source
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::diagnostics::{Position, Range};

    fn create_test_diagnostic(
        file: &str,
        line: u32,
        col: u32,
        severity: crate::diagnostics::Severity,
        message: &str,
    ) -> Diagnostic {
        Diagnostic {
            file: file.to_string(),
            range: Range::new(Position::new(line, col), Position::new(line, col + 10)),
            severity,
            code: None,
            message: message.to_string(),
            source: "test".to_string(),
            related_info: Vec::new(),
        }
    }

    #[test]
    fn test_cache_creation() {
        let temp_dir = TempDir::new().unwrap();
        let cache = DiagnosticCache::new(temp_dir.path().to_path_buf());

        assert_eq!(cache.version, DiagnosticCache::VERSION);
        assert_eq!(cache.entries.len(), 0);
        assert_eq!(cache.metadata.total_files, 0);
    }

    #[test]
    fn test_diagnostic_diff() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache_path = temp_dir.path().join("cache.json");
        let workspace = temp_dir.path().to_path_buf();

        let mut manager = CacheManager::new(workspace, cache_path, true)?;

        // Create some initial diagnostics
        let cached_diagnostics = vec![
            create_test_diagnostic(
                "file1.rs",
                1,
                0,
                crate::diagnostics::Severity::Error,
                "error 1",
            ),
            create_test_diagnostic(
                "file2.rs",
                2,
                0,
                crate::diagnostics::Severity::Warning,
                "warning 1",
            ),
        ];

        // Cache them
        for diag in &cached_diagnostics {
            let test_file = temp_dir.path().join(&diag.file);
            std::fs::write(&test_file, "test content")?;
            manager.cache_diagnostics(&test_file, vec![diag.clone()])?;
        }

        // Create current diagnostics with changes
        let current_diagnostics = vec![
            create_test_diagnostic(
                "file1.rs",
                1,
                0,
                crate::diagnostics::Severity::Error,
                "error 1",
            ), // unchanged
            create_test_diagnostic(
                "file2.rs",
                2,
                0,
                crate::diagnostics::Severity::Warning,
                "warning 1 modified",
            ), // changed
            create_test_diagnostic(
                "file3.rs",
                3,
                0,
                crate::diagnostics::Severity::Info,
                "new info",
            ), // new
        ];

        let diff = manager.compute_diff(&current_diagnostics);

        assert_eq!(diff.summary.unchanged_count, 1);
        assert_eq!(diff.summary.changed_count, 1);
        assert_eq!(diff.summary.new_count, 1);
        assert_eq!(diff.summary.removed_count, 0);

        // Test diff-only output
        let diff_diagnostics = manager.get_diff_diagnostics(&current_diagnostics);
        assert_eq!(diff_diagnostics.len(), 2); // 1 changed + 1 new

        Ok(())
    }

    #[test]
    fn test_cache_manager() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let cache_path = temp_dir.path().join("cache.json");
        let workspace = temp_dir.path().to_path_buf();

        let mut manager = CacheManager::new(workspace, cache_path, true)?;
        assert!(manager.is_enabled());

        // Test stats
        let stats = manager.stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);

        Ok(())
    }

    #[test]
    fn test_diagnostic_key_generation() {
        let diag1 = create_test_diagnostic(
            "file1.rs",
            10,
            5,
            crate::diagnostics::Severity::Error,
            "error",
        );
        let diag2 = create_test_diagnostic(
            "file1.rs",
            10,
            5,
            crate::diagnostics::Severity::Error,
            "different message",
        );
        let diag3 = create_test_diagnostic(
            "file1.rs",
            11,
            5,
            crate::diagnostics::Severity::Error,
            "error",
        );

        let key1 = diagnostic_key(&diag1);
        let key2 = diagnostic_key(&diag2);
        let key3 = diagnostic_key(&diag3);

        assert_eq!(key1, key2); // Same position and severity, different message -> same key
        assert_ne!(key1, key3); // Different line -> different key
    }

    #[test]
    fn test_diagnostics_equal() {
        let diag1 = create_test_diagnostic(
            "file1.rs",
            10,
            5,
            crate::diagnostics::Severity::Error,
            "error",
        );
        let diag2 = create_test_diagnostic(
            "file1.rs",
            10,
            5,
            crate::diagnostics::Severity::Error,
            "error",
        );
        let diag3 = create_test_diagnostic(
            "file1.rs",
            10,
            5,
            crate::diagnostics::Severity::Error,
            "different error",
        );

        assert!(diagnostics_equal(&diag1, &diag2));
        assert!(!diagnostics_equal(&diag1, &diag3));
    }

    #[test]
    fn test_file_hashing() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn main() {}")?;

        let cache = DiagnosticCache::new(temp_dir.path().to_path_buf());
        let hash1 = cache.calculate_file_hash(&test_file)?;

        // Same content should produce same hash
        let hash2 = cache.calculate_file_hash(&test_file)?;
        assert_eq!(hash1, hash2);

        // Different content should produce different hash
        fs::write(&test_file, "fn main() { println!(\"hello\"); }")?;
        let hash3 = cache.calculate_file_hash(&test_file)?;
        assert_ne!(hash1, hash3);

        Ok(())
    }
}
