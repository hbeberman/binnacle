/**
 * Info Panel Component
 * 
 * Displays detailed information about a selected node with:
 * - Header with node ID and title
 * - Tabbed interface: Details, Activity, Commits
 * - Node-specific content in each tab
 */

const INFO_PANEL_ACTIVE_TAB_KEY = 'binnacle_info_panel_active_tab';

/**
 * Load last active tab from localStorage
 * @returns {string} Tab name (details, activity, or commits)
 */
function loadActiveTab() {
    try {
        return localStorage.getItem(INFO_PANEL_ACTIVE_TAB_KEY) || 'details';
    } catch {
        return 'details';
    }
}

/**
 * Save active tab to localStorage
 * @param {string} tabName - Tab name to save
 */
function saveActiveTab(tabName) {
    try {
        localStorage.setItem(INFO_PANEL_ACTIVE_TAB_KEY, tabName);
    } catch {
        // Ignore localStorage errors
    }
}

/**
 * Create info panel HTML structure
 * @returns {HTMLElement} Info panel container element
 */
export function createInfoPanel() {
    const panel = document.createElement('div');
    panel.className = 'info-panel';
    panel.id = 'info-panel';
    
    panel.innerHTML = `
        <div class="info-panel-header">
            <span class="info-panel-title">Node Info</span>
            <button class="info-panel-close" id="info-panel-close" title="Close">&times;</button>
        </div>
        <div id="info-panel-id" class="info-panel-id"></div>
        <div id="info-panel-task-title" class="info-panel-task-title"></div>
        <div id="info-panel-short-name-row" class="info-panel-short-name-row" style="display: none;">
            <span class="info-panel-label">Display Name:</span>
            <span id="info-panel-short-name" class="info-panel-short-name"></span>
        </div>
        <div id="info-panel-meta" class="info-panel-meta"></div>
        <div class="info-panel-tabs">
            <button class="info-panel-tab active" data-tab="details">Details</button>
            <button class="info-panel-tab" data-tab="activity">Activity</button>
            <button class="info-panel-tab" data-tab="commits">Commits</button>
        </div>
        <div id="info-panel-details-tab" class="info-panel-tab-content active">
            <div id="info-panel-queue-section" class="info-panel-queue-section" style="display: none;">
                <span class="info-panel-queue-label">In Queue</span>
                <button id="queue-toggle-btn" class="queue-toggle-switch" title="Toggle queue membership"></button>
            </div>
            <div id="info-panel-doc-open-section" class="info-panel-doc-open-section" style="display: none;">
                <span class="info-panel-doc-open-label">View Document</span>
                <button id="doc-open-btn" class="doc-open-btn" title="Open document viewer">Open</button>
            </div>
            <div id="info-panel-description-section" class="info-panel-section">
                <div class="info-panel-section-title">Description</div>
                <div id="info-panel-description" class="info-panel-description"></div>
            </div>
            <div id="info-panel-deps-section" class="info-panel-section" style="display: none;">
                <div class="info-panel-section-title">Depends On</div>
                <ul id="info-panel-deps" class="info-panel-deps"></ul>
            </div>
            <div id="info-panel-relationships-section" class="info-panel-section" style="display: none;">
                <div class="info-panel-section-title">Relationships</div>
                <ul id="info-panel-relationships" class="info-panel-relationships"></ul>
            </div>
            <div id="info-panel-closed-section" class="info-panel-section" style="display: none;">
                <div class="info-panel-section-title">Closed Reason</div>
                <div id="info-panel-closed-reason" class="info-panel-closed-reason"></div>
            </div>
            <div id="info-panel-timestamps-section" class="info-panel-section" style="display: none;">
                <div class="info-panel-section-title">Timestamps</div>
                <dl id="info-panel-timestamps" class="info-panel-timestamps"></dl>
            </div>
        </div>
        <div id="info-panel-activity-tab" class="info-panel-tab-content">
            <div id="info-panel-activity-content" class="info-panel-activity-log">
                <div class="info-panel-activity-loading">Loading activity...</div>
            </div>
            <a id="info-panel-view-full-log" class="info-panel-view-full-log">View in full activity log →</a>
        </div>
        <div id="info-panel-commits-tab" class="info-panel-tab-content">
            <div id="info-panel-commits-content" class="info-panel-commits-log">
                <div class="info-panel-commits-loading">Loading commits...</div>
            </div>
        </div>
    `;
    
    return panel;
}

/**
 * Initialize info panel event handlers
 * @param {HTMLElement} panel - The info panel element
 * @param {Object} options - Configuration options
 * @param {Function} options.onClose - Callback when panel is closed
 * @param {Function} options.onTabChange - Callback when tab changes (tabName)
 * @param {Function} options.onQueueToggle - Callback when queue toggle is clicked
 * @param {Function} options.onDocOpen - Callback when doc open is clicked
 * @param {Function} options.onViewFullLog - Callback when view full log is clicked
 */
export function initializeInfoPanel(panel, options = {}) {
    const {
        onClose = () => {},
        onTabChange = () => {},
        onQueueToggle = () => {},
        onDocOpen = () => {},
        onViewFullLog = () => {}
    } = options;
    
    // Close button
    const closeBtn = panel.querySelector('#info-panel-close');
    closeBtn.addEventListener('click', () => {
        hideInfoPanel(panel);
        onClose();
    });
    
    // Tab switching
    const tabs = panel.querySelectorAll('.info-panel-tab');
    tabs.forEach(tab => {
        tab.addEventListener('click', () => {
            const tabName = tab.dataset.tab;
            switchTab(panel, tabName);
            saveActiveTab(tabName);
            onTabChange(tabName);
        });
    });
    
    // Queue toggle button
    const queueToggleBtn = panel.querySelector('#queue-toggle-btn');
    if (queueToggleBtn) {
        queueToggleBtn.addEventListener('click', () => {
            onQueueToggle();
        });
    }
    
    // Doc open button
    const docOpenBtn = panel.querySelector('#doc-open-btn');
    if (docOpenBtn) {
        docOpenBtn.addEventListener('click', () => {
            onDocOpen();
        });
    }
    
    // View full log link
    const viewFullLogLink = panel.querySelector('#info-panel-view-full-log');
    if (viewFullLogLink) {
        viewFullLogLink.addEventListener('click', (e) => {
            e.preventDefault();
            onViewFullLog();
        });
    }
    
    // Restore last active tab
    const lastTab = loadActiveTab();
    switchTab(panel, lastTab);
}

/**
 * Switch to a specific tab
 * @param {HTMLElement} panel - The info panel element
 * @param {string} tabName - Tab name to switch to (details, activity, commits)
 */
function switchTab(panel, tabName) {
    // Update tab buttons
    panel.querySelectorAll('.info-panel-tab').forEach(tab => {
        tab.classList.toggle('active', tab.dataset.tab === tabName);
    });
    
    // Update tab content
    panel.querySelectorAll('.info-panel-tab-content').forEach(content => {
        content.classList.remove('active');
    });
    const targetContent = panel.querySelector(`#info-panel-${tabName}-tab`);
    if (targetContent) {
        targetContent.classList.add('active');
    }
}

/**
 * Show the info panel
 * @param {HTMLElement} panel - The info panel element
 */
export function showInfoPanel(panel) {
    panel.classList.add('visible');
}

/**
 * Hide the info panel
 * @param {HTMLElement} panel - The info panel element
 */
export function hideInfoPanel(panel) {
    panel.classList.remove('visible');
}

/**
 * Update info panel content for a node
 * @param {HTMLElement} panel - The info panel element
 * @param {Object} node - Node data object
 */
export function updateInfoPanelContent(panel, node) {
    if (!node) {
        hideInfoPanel(panel);
        return;
    }
    
    // Update ID
    const idEl = panel.querySelector('#info-panel-id');
    idEl.textContent = node.id || '';
    
    // Update title
    const titleEl = panel.querySelector('#info-panel-task-title');
    titleEl.textContent = node.title || '';
    
    // Update short name
    const shortNameRow = panel.querySelector('#info-panel-short-name-row');
    const shortNameEl = panel.querySelector('#info-panel-short-name');
    if (node.short_name) {
        shortNameRow.style.display = 'block';
        shortNameEl.textContent = node.short_name;
    } else {
        shortNameRow.style.display = 'none';
    }
    
    // Update meta (status, priority, etc.)
    const metaEl = panel.querySelector('#info-panel-meta');
    const metaParts = [];
    if (node.status) metaParts.push(`Status: ${node.status}`);
    if (node.priority !== undefined && node.priority !== null) metaParts.push(`Priority: ${node.priority}`);
    if (node.type) metaParts.push(`Type: ${node.type}`);
    metaEl.textContent = metaParts.join(' • ');
    
    // Update description
    const descEl = panel.querySelector('#info-panel-description');
    if (node.description) {
        descEl.textContent = node.description;
        descEl.classList.remove('empty');
    } else {
        descEl.textContent = 'No description provided.';
        descEl.classList.add('empty');
    }
    
    // Update dependencies (if any)
    const depsSection = panel.querySelector('#info-panel-deps-section');
    const depsEl = panel.querySelector('#info-panel-deps');
    if (node.depends_on && node.depends_on.length > 0) {
        depsSection.style.display = 'block';
        depsEl.innerHTML = node.depends_on.map(dep => `<li>${dep}</li>`).join('');
    } else {
        depsSection.style.display = 'none';
    }
    
    // Update closed reason (if closed)
    const closedSection = panel.querySelector('#info-panel-closed-section');
    const closedReasonEl = panel.querySelector('#info-panel-closed-reason');
    if (node.status === 'done' && node.closed_reason) {
        closedSection.style.display = 'block';
        closedReasonEl.textContent = node.closed_reason;
    } else {
        closedSection.style.display = 'none';
    }
    
    // Update queue section (task/bug only)
    const queueSection = panel.querySelector('#info-panel-queue-section');
    if (node.type === 'task' || node.type === 'bug') {
        queueSection.style.display = 'flex';
        const queueBtn = panel.querySelector('#queue-toggle-btn');
        queueBtn.classList.toggle('active', node.queued === true);
    } else {
        queueSection.style.display = 'none';
    }
    
    // Update doc open section (doc only)
    const docSection = panel.querySelector('#info-panel-doc-open-section');
    if (node.type === 'doc') {
        docSection.style.display = 'flex';
    } else {
        docSection.style.display = 'none';
    }
    
    // Show panel
    showInfoPanel(panel);
}

/**
 * Update activity tab content
 * @param {HTMLElement} panel - The info panel element
 * @param {Array} activities - Array of activity entries
 */
export function updateActivityTab(panel, activities) {
    const contentEl = panel.querySelector('#info-panel-activity-content');
    
    if (!activities || activities.length === 0) {
        contentEl.innerHTML = '<div class="info-panel-activity-empty">No activity recorded.</div>';
        return;
    }
    
    const activityHTML = activities.map(activity => `
        <div class="info-panel-activity-entry">
            <div class="info-panel-activity-time">${activity.timestamp || ''}</div>
            <div class="info-panel-activity-message">${activity.message || ''}</div>
        </div>
    `).join('');
    
    contentEl.innerHTML = activityHTML;
}

/**
 * Update commits tab content
 * @param {HTMLElement} panel - The info panel element
 * @param {Array} commits - Array of commit entries
 */
export function updateCommitsTab(panel, commits) {
    const contentEl = panel.querySelector('#info-panel-commits-content');
    
    if (!commits || commits.length === 0) {
        contentEl.innerHTML = '<div class="info-panel-commits-empty">No commits linked.</div>';
        return;
    }
    
    const commitsHTML = commits.map(commit => `
        <div class="info-panel-commit-entry">
            <div class="info-panel-commit-sha">${commit.sha || ''}</div>
            <div class="info-panel-commit-message">${commit.message || ''}</div>
        </div>
    `).join('');
    
    contentEl.innerHTML = commitsHTML;
}
