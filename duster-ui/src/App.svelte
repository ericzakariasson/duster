<script>
  import { invoke } from "@tauri-apps/api/core";

  // State
  let view = $state("home"); // home, scanning, results, cleaning
  let scanResult = $state(null);
  let selectedCategories = $state(new Set());
  let selectedFiles = $state(new Set());
  let isScanning = $state(false);
  let isCleaning = $state(false);
  let cleanupResult = $state(null);
  let error = $state(null);
  let scanProgress = $state(0);
  
  // Categories with icons
  const categoryIcons = {
    cache: "🗄️",
    trash: "🗑️",
    temp: "⏳",
    downloads: "📥",
    build: "🔨",
    large: "📦",
    duplicates: "👯",
    old: "📜"
  };
  
  const categoryColors = {
    cache: "#6366f1",
    trash: "#ef4444",
    temp: "#f59e0b",
    downloads: "#10b981",
    build: "#8b5cf6",
    large: "#ec4899",
    duplicates: "#06b6d4",
    old: "#84cc16"
  };

  // Toggle category selection
  function toggleCategory(key) {
    const newSet = new Set(selectedCategories);
    if (newSet.has(key)) {
      newSet.delete(key);
    } else {
      newSet.add(key);
    }
    selectedCategories = newSet;
  }
  
  // Toggle file selection
  function toggleFile(path) {
    const newSet = new Set(selectedFiles);
    if (newSet.has(path)) {
      newSet.delete(path);
    } else {
      newSet.add(path);
    }
    selectedFiles = newSet;
  }
  
  // Select all files in category
  function selectAllInCategory(categoryKey) {
    const newSet = new Set(selectedFiles);
    const files = scanResult?.files.filter(f => f.category_key === categoryKey) || [];
    files.forEach(f => newSet.add(f.path));
    selectedFiles = newSet;
  }
  
  // Deselect all files in category
  function deselectAllInCategory(categoryKey) {
    const newSet = new Set(selectedFiles);
    const files = scanResult?.files.filter(f => f.category_key === categoryKey) || [];
    files.forEach(f => newSet.delete(f.path));
    selectedFiles = newSet;
  }

  // Run scan
  async function startScan() {
    isScanning = true;
    view = "scanning";
    error = null;
    scanProgress = 0;
    
    // Animate progress
    const progressInterval = setInterval(() => {
      if (scanProgress < 90) {
        scanProgress += Math.random() * 15;
      }
    }, 200);
    
    try {
      const options = {
        categories: Array.from(selectedCategories),
        min_age: null,
        min_size_mb: null,
        project_age: null,
        path: null
      };
      
      const result = await invoke("run_scan", { options });
      scanProgress = 100;
      
      setTimeout(() => {
        scanResult = result;
        // Auto-select all files
        selectedFiles = new Set(result.files.map(f => f.path));
        view = "results";
        isScanning = false;
      }, 300);
      
    } catch (e) {
      error = e.toString();
      view = "home";
      isScanning = false;
    } finally {
      clearInterval(progressInterval);
    }
  }
  
  // Clean selected files
  async function startClean() {
    if (selectedFiles.size === 0) return;
    
    isCleaning = true;
    view = "cleaning";
    error = null;
    
    try {
      const paths = Array.from(selectedFiles);
      const result = await invoke("delete_files", { paths });
      cleanupResult = result;
      view = "done";
    } catch (e) {
      error = e.toString();
      view = "results";
    } finally {
      isCleaning = false;
    }
  }
  
  // Reset to home
  function goHome() {
    view = "home";
    scanResult = null;
    selectedFiles = new Set();
    cleanupResult = null;
    error = null;
    scanProgress = 0;
  }
  
  // Get selected size
  function getSelectedSize() {
    if (!scanResult) return 0;
    return scanResult.files
      .filter(f => selectedFiles.has(f.path))
      .reduce((acc, f) => acc + f.size, 0);
  }
  
  function formatSize(bytes) {
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    let i = 0;
    while (bytes >= 1024 && i < units.length - 1) {
      bytes /= 1024;
      i++;
    }
    return `${bytes.toFixed(i > 0 ? 2 : 0)} ${units[i]}`;
  }
  
  // Group files by category for display
  function getFilesByCategory() {
    if (!scanResult) return [];
    const groups = {};
    for (const file of scanResult.files) {
      if (!groups[file.category_key]) {
        groups[file.category_key] = {
          key: file.category_key,
          name: file.category,
          files: [],
          totalSize: 0
        };
      }
      groups[file.category_key].files.push(file);
      groups[file.category_key].totalSize += file.size;
    }
    return Object.values(groups).sort((a, b) => b.totalSize - a.totalSize);
  }
  
  // Expanded categories state
  let expandedCategories = $state(new Set());
  
  function toggleExpand(key) {
    const newSet = new Set(expandedCategories);
    if (newSet.has(key)) {
      newSet.delete(key);
    } else {
      newSet.add(key);
    }
    expandedCategories = newSet;
  }
</script>

<main>
  <!-- Animated background particles -->
  <div class="particles">
    {#each Array(20) as _, i}
      <div class="particle" style="--delay: {i * 0.5}s; --x: {Math.random() * 100}%; --size: {2 + Math.random() * 4}px;"></div>
    {/each}
  </div>
  
  <div class="container">
    <!-- Header -->
    <header>
      <button class="logo" onclick={goHome}>
        <span class="logo-icon">✨</span>
        <span class="logo-text">duster</span>
      </button>
      {#if view !== "home" && view !== "scanning"}
        <button class="back-btn" onclick={goHome}>
          ← Back
        </button>
      {/if}
    </header>
    
    <!-- Home View -->
    {#if view === "home"}
      <div class="home" style="animation: fadeIn 0.5s ease-out">
        <div class="hero">
          <h1>Clean up your disk</h1>
          <p class="subtitle">Find and remove build artifacts, caches, and other space hogs</p>
        </div>
        
        <div class="category-grid">
          {#each [
            { key: "cache", name: "System Cache", desc: "App and system caches" },
            { key: "trash", name: "Trash", desc: "Files in trash bin" },
            { key: "temp", name: "Temp Files", desc: "Temporary files" },
            { key: "downloads", name: "Downloads", desc: "Old downloaded files" },
            { key: "build", name: "Build Artifacts", desc: "node_modules, target/, etc." },
            { key: "large", name: "Large Files", desc: "Files over 100MB" },
            { key: "duplicates", name: "Duplicates", desc: "Duplicate files" },
            { key: "old", name: "Old Files", desc: "Unused for 30+ days" }
          ] as cat, i}
            <button 
              class="category-card"
              class:selected={selectedCategories.has(cat.key)}
              onclick={() => toggleCategory(cat.key)}
              style="animation-delay: {i * 0.05}s; --accent: {categoryColors[cat.key]}"
            >
              <span class="cat-icon">{categoryIcons[cat.key]}</span>
              <span class="cat-name">{cat.name}</span>
              <span class="cat-desc">{cat.desc}</span>
              <div class="check-indicator">
                {#if selectedCategories.has(cat.key)}
                  ✓
                {/if}
              </div>
            </button>
          {/each}
        </div>
        
        <div class="actions">
          <button class="scan-btn" onclick={startScan}>
            <span class="btn-icon">🔍</span>
            {selectedCategories.size === 0 ? "Scan All" : `Scan ${selectedCategories.size} Categories`}
          </button>
        </div>
        
        {#if error}
          <div class="error-toast">{error}</div>
        {/if}
      </div>
    {/if}
    
    <!-- Scanning View -->
    {#if view === "scanning"}
      <div class="scanning">
        <div class="scan-animation">
          <div class="scan-ring"></div>
          <div class="scan-ring delay-1"></div>
          <div class="scan-ring delay-2"></div>
          <div class="scan-icon">🔍</div>
        </div>
        <h2>Scanning your disk...</h2>
        <div class="progress-bar">
          <div class="progress-fill" style="width: {Math.min(scanProgress, 100)}%"></div>
        </div>
        <p class="scan-status">Finding cleanable files</p>
      </div>
    {/if}
    
    <!-- Results View -->
    {#if view === "results" && scanResult}
      <div class="results" style="animation: fadeIn 0.4s ease-out">
        <div class="results-header">
          <div class="total-found">
            <span class="total-size">{formatSize(scanResult.total_size)}</span>
            <span class="total-label">can be freed</span>
          </div>
          <div class="total-files">{scanResult.total_count} files found</div>
        </div>
        
        <div class="results-body">
          {#each getFilesByCategory() as group, i}
            <div class="result-category" style="animation-delay: {i * 0.08}s; --accent: {categoryColors[group.key]}">
              <div class="category-header">
                <button class="category-toggle" onclick={() => toggleExpand(group.key)}>
                  <span class="cat-icon">{categoryIcons[group.key]}</span>
                  <span class="cat-info">
                    <span class="cat-name">{group.name}</span>
                    <span class="cat-stats">{group.files.length} items · {formatSize(group.totalSize)}</span>
                  </span>
                  <span class="expand-icon">{expandedCategories.has(group.key) ? '−' : '+'}</span>
                </button>
                <div class="select-actions">
                  <button class="select-all" onclick={() => selectAllInCategory(group.key)}>All</button>
                  <button class="select-none" onclick={() => deselectAllInCategory(group.key)}>None</button>
                </div>
              </div>
              
              {#if expandedCategories.has(group.key)}
                <div class="file-list">
                  {#each group.files.slice(0, 50) as file}
                    <label class="file-item" class:selected={selectedFiles.has(file.path)}>
                      <input 
                        type="checkbox" 
                        checked={selectedFiles.has(file.path)}
                        onchange={() => toggleFile(file.path)}
                      />
                      <span class="file-path" title={file.path}>{file.path}</span>
                      <span class="file-size">{file.size_formatted}</span>
                    </label>
                  {/each}
                  {#if group.files.length > 50}
                    <div class="more-files">... and {group.files.length - 50} more</div>
                  {/if}
                </div>
              {/if}
            </div>
          {/each}
        </div>
        
        <div class="results-footer">
          <div class="selected-info">
            <span class="selected-size">{formatSize(getSelectedSize())}</span>
            <span class="selected-label">selected ({selectedFiles.size} files)</span>
          </div>
          <button 
            class="clean-btn" 
            onclick={startClean}
            disabled={selectedFiles.size === 0}
          >
            🧹 Clean Selected
          </button>
        </div>
      </div>
    {/if}
    
    <!-- Cleaning View -->
    {#if view === "cleaning"}
      <div class="cleaning">
        <div class="clean-animation">
          <span class="clean-icon">🧹</span>
        </div>
        <h2>Cleaning up...</h2>
        <p>Removing {selectedFiles.size} items</p>
      </div>
    {/if}
    
    <!-- Done View -->
    {#if view === "done" && cleanupResult}
      <div class="done" style="animation: fadeIn 0.4s ease-out">
        <div class="done-icon">✨</div>
        <h2>All clean!</h2>
        <div class="done-stats">
          <div class="stat">
            <span class="stat-value">{cleanupResult.freed_formatted}</span>
            <span class="stat-label">freed</span>
          </div>
          <div class="stat">
            <span class="stat-value">{cleanupResult.deleted_count}</span>
            <span class="stat-label">items removed</span>
          </div>
        </div>
        {#if cleanupResult.errors.length > 0}
          <div class="errors-list">
            <p>{cleanupResult.errors.length} items couldn't be removed:</p>
            {#each cleanupResult.errors.slice(0, 5) as err}
              <div class="error-item">{err}</div>
            {/each}
          </div>
        {/if}
        <button class="scan-btn" onclick={goHome}>
          Scan Again
        </button>
      </div>
    {/if}
  </div>
</main>

<style>
  :global(*) {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
  }
  
  :global(body) {
    font-family: 'Outfit', -apple-system, BlinkMacSystemFont, sans-serif;
    background: #0a0a0f;
    color: #e4e4e7;
    min-height: 100vh;
    overflow-x: hidden;
  }
  
  main {
    min-height: 100vh;
    position: relative;
  }
  
  /* Particles */
  .particles {
    position: fixed;
    inset: 0;
    pointer-events: none;
    overflow: hidden;
  }
  
  .particle {
    position: absolute;
    width: var(--size);
    height: var(--size);
    background: rgba(251, 146, 60, 0.3);
    border-radius: 50%;
    left: var(--x);
    animation: float 15s ease-in-out infinite;
    animation-delay: var(--delay);
    filter: blur(1px);
  }
  
  @keyframes float {
    0%, 100% { transform: translateY(100vh) rotate(0deg); opacity: 0; }
    10% { opacity: 1; }
    90% { opacity: 1; }
    100% { transform: translateY(-10vh) rotate(360deg); opacity: 0; }
  }
  
  @keyframes fadeIn {
    from { opacity: 0; transform: translateY(10px); }
    to { opacity: 1; transform: translateY(0); }
  }
  
  .container {
    max-width: 900px;
    margin: 0 auto;
    padding: 2rem;
    position: relative;
    z-index: 1;
  }
  
  /* Header */
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 3rem;
  }
  
  .logo {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: none;
    border: none;
    cursor: pointer;
    padding: 0.5rem;
  }
  
  .logo-icon {
    font-size: 1.5rem;
  }
  
  .logo-text {
    font-size: 1.5rem;
    font-weight: 600;
    background: linear-gradient(135deg, #fb923c, #f97316);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }
  
  .back-btn {
    background: rgba(255,255,255,0.05);
    border: 1px solid rgba(255,255,255,0.1);
    color: #a1a1aa;
    padding: 0.5rem 1rem;
    border-radius: 8px;
    cursor: pointer;
    font-family: inherit;
    transition: all 0.2s;
  }
  
  .back-btn:hover {
    background: rgba(255,255,255,0.1);
    color: #e4e4e7;
  }
  
  /* Home View */
  .hero {
    text-align: center;
    margin-bottom: 3rem;
  }
  
  .hero h1 {
    font-size: 2.5rem;
    font-weight: 700;
    margin-bottom: 0.5rem;
    background: linear-gradient(135deg, #fff 0%, #a1a1aa 100%);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }
  
  .subtitle {
    color: #71717a;
    font-size: 1.1rem;
  }
  
  /* Category Grid */
  .category-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 1rem;
    margin-bottom: 2rem;
  }
  
  .category-card {
    background: rgba(255,255,255,0.03);
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 16px;
    padding: 1.25rem;
    cursor: pointer;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.5rem;
    position: relative;
    overflow: hidden;
    text-align: left;
    font-family: inherit;
    color: inherit;
    animation: fadeIn 0.5s ease-out backwards;
  }
  
  .category-card::before {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(135deg, var(--accent), transparent);
    opacity: 0;
    transition: opacity 0.3s;
  }
  
  .category-card:hover {
    transform: translateY(-2px);
    border-color: rgba(255,255,255,0.15);
  }
  
  .category-card:hover::before {
    opacity: 0.1;
  }
  
  .category-card.selected {
    border-color: var(--accent);
    background: rgba(255,255,255,0.05);
  }
  
  .category-card.selected::before {
    opacity: 0.15;
  }
  
  .cat-icon {
    font-size: 1.5rem;
    position: relative;
    z-index: 1;
  }
  
  .cat-name {
    font-weight: 600;
    font-size: 1rem;
    position: relative;
    z-index: 1;
  }
  
  .cat-desc {
    font-size: 0.8rem;
    color: #71717a;
    position: relative;
    z-index: 1;
  }
  
  .check-indicator {
    position: absolute;
    top: 0.75rem;
    right: 0.75rem;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--accent);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.7rem;
    font-weight: bold;
    opacity: 0;
    transform: scale(0);
    transition: all 0.2s;
  }
  
  .category-card.selected .check-indicator {
    opacity: 1;
    transform: scale(1);
  }
  
  /* Actions */
  .actions {
    display: flex;
    justify-content: center;
  }
  
  .scan-btn, .clean-btn {
    background: linear-gradient(135deg, #fb923c, #ea580c);
    border: none;
    color: white;
    padding: 1rem 2.5rem;
    border-radius: 12px;
    font-size: 1.1rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.3s;
    font-family: inherit;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    box-shadow: 0 4px 20px rgba(251, 146, 60, 0.3);
  }
  
  .scan-btn:hover, .clean-btn:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 30px rgba(251, 146, 60, 0.4);
  }
  
  .clean-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none;
  }
  
  /* Scanning View */
  .scanning, .cleaning, .done {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 60vh;
    text-align: center;
  }
  
  .scan-animation {
    position: relative;
    width: 120px;
    height: 120px;
    margin-bottom: 2rem;
  }
  
  .scan-ring {
    position: absolute;
    inset: 0;
    border: 2px solid rgba(251, 146, 60, 0.3);
    border-top-color: #fb923c;
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }
  
  .scan-ring.delay-1 {
    inset: 10px;
    animation-delay: 0.2s;
    border-color: rgba(251, 146, 60, 0.2);
    border-top-color: #f97316;
  }
  
  .scan-ring.delay-2 {
    inset: 20px;
    animation-delay: 0.4s;
    border-color: rgba(251, 146, 60, 0.1);
    border-top-color: #ea580c;
  }
  
  @keyframes spin {
    to { transform: rotate(360deg); }
  }
  
  .scan-icon, .clean-icon, .done-icon {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    font-size: 2rem;
  }
  
  .scanning h2, .cleaning h2, .done h2 {
    font-size: 1.5rem;
    margin-bottom: 1rem;
  }
  
  .progress-bar {
    width: 300px;
    height: 6px;
    background: rgba(255,255,255,0.1);
    border-radius: 3px;
    overflow: hidden;
    margin-bottom: 1rem;
  }
  
  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #fb923c, #f97316);
    transition: width 0.3s ease-out;
  }
  
  .scan-status {
    color: #71717a;
  }
  
  /* Clean animation */
  .clean-animation {
    font-size: 4rem;
    animation: bounce 0.6s ease-in-out infinite;
    margin-bottom: 1rem;
  }
  
  @keyframes bounce {
    0%, 100% { transform: translateY(0) rotate(0deg); }
    50% { transform: translateY(-10px) rotate(10deg); }
  }
  
  /* Done View */
  .done-icon {
    font-size: 4rem;
    margin-bottom: 1rem;
    animation: pop 0.5s cubic-bezier(0.68, -0.55, 0.265, 1.55);
    position: relative;
  }
  
  @keyframes pop {
    0% { transform: scale(0); }
    100% { transform: scale(1); }
  }
  
  .done-stats {
    display: flex;
    gap: 3rem;
    margin: 2rem 0;
  }
  
  .stat {
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  
  .stat-value {
    font-size: 2rem;
    font-weight: 700;
    color: #fb923c;
  }
  
  .stat-label {
    color: #71717a;
    font-size: 0.9rem;
  }
  
  /* Results View */
  .results {
    display: flex;
    flex-direction: column;
    height: calc(100vh - 150px);
  }
  
  .results-header {
    text-align: center;
    margin-bottom: 2rem;
    padding: 1.5rem;
    background: rgba(251, 146, 60, 0.1);
    border-radius: 16px;
    border: 1px solid rgba(251, 146, 60, 0.2);
  }
  
  .total-size {
    font-size: 2.5rem;
    font-weight: 700;
    background: linear-gradient(135deg, #fb923c, #f97316);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }
  
  .total-label {
    font-size: 1.2rem;
    color: #a1a1aa;
    margin-left: 0.5rem;
  }
  
  .total-files {
    color: #71717a;
    margin-top: 0.25rem;
  }
  
  .results-body {
    flex: 1;
    overflow-y: auto;
    padding-right: 0.5rem;
  }
  
  .results-body::-webkit-scrollbar {
    width: 6px;
  }
  
  .results-body::-webkit-scrollbar-track {
    background: rgba(255,255,255,0.05);
    border-radius: 3px;
  }
  
  .results-body::-webkit-scrollbar-thumb {
    background: rgba(255,255,255,0.2);
    border-radius: 3px;
  }
  
  .result-category {
    margin-bottom: 0.75rem;
    background: rgba(255,255,255,0.02);
    border: 1px solid rgba(255,255,255,0.06);
    border-radius: 12px;
    overflow: hidden;
    animation: fadeIn 0.4s ease-out backwards;
  }
  
  .category-header {
    display: flex;
    align-items: center;
    padding: 0.5rem;
    padding-left: 0;
  }
  
  .category-toggle {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.5rem 1rem;
    background: none;
    border: none;
    color: inherit;
    font-family: inherit;
    cursor: pointer;
    transition: background 0.2s;
    text-align: left;
    border-radius: 8px;
  }
  
  .category-toggle:hover {
    background: rgba(255,255,255,0.03);
  }
  
  .cat-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  
  .cat-stats {
    font-size: 0.85rem;
    color: #71717a;
  }
  
  .expand-icon {
    font-size: 1.2rem;
    color: #71717a;
    width: 24px;
    text-align: center;
  }
  
  .select-actions {
    display: flex;
    gap: 0.5rem;
  }
  
  .select-all, .select-none {
    padding: 0.25rem 0.75rem;
    font-size: 0.75rem;
    background: rgba(255,255,255,0.05);
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 6px;
    color: #a1a1aa;
    cursor: pointer;
    font-family: inherit;
    transition: all 0.2s;
  }
  
  .select-all:hover, .select-none:hover {
    background: rgba(255,255,255,0.1);
    color: #e4e4e7;
  }
  
  .file-list {
    border-top: 1px solid rgba(255,255,255,0.06);
    max-height: 300px;
    overflow-y: auto;
  }
  
  .file-item {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.6rem 1rem;
    cursor: pointer;
    transition: background 0.15s;
    border-bottom: 1px solid rgba(255,255,255,0.03);
  }
  
  .file-item:last-child {
    border-bottom: none;
  }
  
  .file-item:hover {
    background: rgba(255,255,255,0.03);
  }
  
  .file-item.selected {
    background: rgba(var(--accent-rgb, 251, 146, 60), 0.1);
  }
  
  .file-item input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: #fb923c;
  }
  
  .file-path {
    flex: 1;
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.8rem;
    color: #a1a1aa;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  
  .file-size {
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.75rem;
    color: #71717a;
    white-space: nowrap;
  }
  
  .more-files {
    padding: 0.75rem 1rem;
    color: #71717a;
    font-size: 0.85rem;
    text-align: center;
    font-style: italic;
  }
  
  /* Results Footer */
  .results-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1.25rem;
    margin-top: 1rem;
    background: rgba(255,255,255,0.03);
    border-radius: 12px;
    border: 1px solid rgba(255,255,255,0.08);
  }
  
  .selected-info {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }
  
  .selected-size {
    font-size: 1.5rem;
    font-weight: 700;
    color: #fb923c;
  }
  
  .selected-label {
    color: #71717a;
  }
  
  /* Error */
  .error-toast {
    position: fixed;
    bottom: 2rem;
    left: 50%;
    transform: translateX(-50%);
    background: #ef4444;
    color: white;
    padding: 1rem 2rem;
    border-radius: 8px;
    animation: slideUp 0.3s ease-out;
  }
  
  @keyframes slideUp {
    from { transform: translateX(-50%) translateY(100%); opacity: 0; }
    to { transform: translateX(-50%) translateY(0); opacity: 1; }
  }
  
  .errors-list {
    margin: 1rem 0;
    padding: 1rem;
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.2);
    border-radius: 8px;
    font-size: 0.85rem;
    max-width: 500px;
    text-align: left;
  }
  
  .error-item {
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.75rem;
    color: #fca5a5;
    margin-top: 0.5rem;
    word-break: break-all;
  }
</style>
