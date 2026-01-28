/**
 * Info Panel Component
 * 
 * Displays detailed information about a selected node with:
 * - Header with node ID and title
 * - Tabbed interface: Details, Activity, Commits
 * - Node-specific content in each tab
 */

import { createClickableId } from '../utils/clickable-ids.js';

/**
 * Format a date timestamp for display
 * @param {string} timestamp - ISO timestamp
 * @returns {string} Formatted date
 */
function formatDate(timestamp) {
    if (!timestamp) return 'N/A';
    const date = new Date(timestamp);
    return date.toLocaleDateString() + ' ' + date.toLocaleTimeString();
}

/**
 * Escape HTML to prevent XSS
 * @param {string} text - Text to escape
 * @returns {string} Escaped text
 */
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

const INFO_PANEL_ACTIVE_TAB_KEY = 'binnacle_info_panel_active_tab';

/**
 * Format edge type name for display
 * @param {string} edgeType - Raw edge type
 * @returns {string} Formatted type name
 */
function formatEdgeType(edgeType) {
    const typeNames = {
        'depends_on': 'depends on',
        'blocks': 'blocks',
        'child_of': 'child of',
        'parent_of': 'parent of',
        'related_to': 'related to',
        'tests': 'tests',
        'tested_by': 'tested by',
        'documents': 'documents',
        'documented_by': 'documented by',
        'queued': 'queued',
        'working_on': 'working on',
        'informational': 'informational'
    };
    return typeNames[edgeType] || edgeType.replace(/_/g, ' ');
}

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
            <div class="info-panel-header-controls">
                <button class="info-panel-expand-btn" id="info-panel-expand" title="Expand">▲</button>
                <button class="info-panel-collapse-btn" id="info-panel-collapse" title="Collapse">▼</button>
                <button class="info-panel-close" id="info-panel-close" title="Close">&times;</button>
            </div>
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
                <div class="write-action-container" data-readonly-tooltip="Queue changes unavailable in readonly mode">
                    <button id="queue-toggle-btn" class="queue-toggle-switch write-action" title="Toggle queue membership"></button>
                </div>
            </div>
            <div id="info-panel-doc-open-section" class="info-panel-doc-open-section" style="display: none;">
                <span class="info-panel-doc-open-label">📖 Read Document</span>
                <button id="doc-open-btn" class="doc-open-btn" title="Open document viewer">Open</button>
            </div>
            <div id="info-panel-summary-section" class="info-panel-section" style="display: none;">
                <div class="info-panel-section-title">Summary</div>
                <div id="info-panel-summary-content" class="info-panel-summary-content"></div>
            </div>
            <div id="info-panel-description-section" class="info-panel-section">
                <div class="info-panel-section-title">Description</div>
                <div id="info-panel-description" class="info-panel-description"></div>
            </div>
            <div id="info-panel-tags-section" class="info-panel-section" style="display: none;">
                <div class="info-panel-section-title">Tags</div>
                <div id="info-panel-tags" class="info-panel-tags"></div>
            </div>
            <div id="info-panel-assignee-section" class="info-panel-section" style="display: none;">
                <div class="info-panel-section-title">Assignee</div>
                <div id="info-panel-assignee" class="info-panel-assignee"></div>
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
 * @param {Function} options.onRelationshipClick - Callback when a relationship is clicked (nodeId)
 * @param {Function} options.onSummaryClick - Callback when summary section is clicked (for doc nodes)
 */
export function initializeInfoPanel(panel, options = {}) {
    const {
        onClose = () => {},
        onTabChange = () => {},
        onQueueToggle = () => {},
        onDocOpen = () => {},
        onViewFullLog = () => {},
        onRelationshipClick = () => {},
        onSummaryClick = () => {}
    } = options;
    
    // Store callback reference for relationship clicks
    if (onRelationshipClick) {
        panel.addEventListener('relationship-click', (e) => {
            onRelationshipClick(e.detail.nodeId);
        });
    }
    
    // Close button - collapses when in expanded mode, hides otherwise
    const closeBtn = panel.querySelector('#info-panel-close');
    closeBtn.addEventListener('click', () => {
        if (panel.classList.contains('expanded')) {
            collapseInfoPanel(panel);
        } else {
            hideInfoPanel(panel);
            onClose();
        }
    });
    
    // Expand button
    const expandBtn = panel.querySelector('#info-panel-expand');
    if (expandBtn) {
        expandBtn.addEventListener('click', () => {
            expandInfoPanel(panel);
        });
    }
    
    // Collapse button
    const collapseBtn = panel.querySelector('#info-panel-collapse');
    if (collapseBtn) {
        collapseBtn.addEventListener('click', () => {
            collapseInfoPanel(panel);
        });
    }
    
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
            // Get the current node ID from the panel's data attribute
            const nodeId = panel.dataset.currentNodeId;
            if (nodeId) {
                onQueueToggle(nodeId);
            }
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
    
    // Summary section click handler (for doc nodes)
    const summarySection = panel.querySelector('#info-panel-summary-section');
    if (summarySection) {
        summarySection.addEventListener('click', () => {
            // Only trigger if the section is marked as clickable (doc nodes)
            if (summarySection.classList.contains('clickable')) {
                const nodeId = panel.dataset.currentNodeId;
                if (nodeId) {
                    onSummaryClick(nodeId);
                }
            }
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
    // Start in compact mode if not already expanded
    if (!panel.classList.contains('expanded')) {
        panel.classList.add('compact');
    }
}

/**
 * Hide the info panel
 * @param {HTMLElement} panel - The info panel element
 */
export function hideInfoPanel(panel) {
    panel.classList.remove('visible');
    panel.classList.remove('expanded');
    panel.classList.remove('compact');
}

/**
 * Expand the info panel to full detail view
 * @param {HTMLElement} panel - The info panel element
 */
export function expandInfoPanel(panel) {
    if (!panel.classList.contains('visible')) {
        showInfoPanel(panel);
    }
    panel.classList.remove('compact');
    panel.classList.add('expanded');
}

/**
 * Collapse the info panel to compact view
 * @param {HTMLElement} panel - The info panel element
 */
export function collapseInfoPanel(panel) {
    panel.classList.remove('expanded');
    panel.classList.add('compact');
}

/**
 * Toggle the info panel between expanded and collapsed states
 * @param {HTMLElement} panel - The info panel element
 * @returns {boolean} True if expanded, false if collapsed
 */
export function toggleInfoPanelExpanded(panel) {
    if (panel.classList.contains('expanded')) {
        collapseInfoPanel(panel);
        return false;
    } else {
        expandInfoPanel(panel);
        return true;
    }
}

/**
 * Update info panel to show batch selection view
 * @param {HTMLElement} panel - The info panel element
 * @param {Array} selectedNodes - Array of selected node objects
 */
function updateBatchView(panel, selectedNodes) {
    // Clear current node ID
    panel.dataset.currentNodeId = '';
    
    // Count entities by type
    const typeCounts = {};
    selectedNodes.forEach(node => {
        const type = node.type || 'unknown';
        typeCounts[type] = (typeCounts[type] || 0) + 1;
    });
    
    // Build summary string (e.g., "3 tasks, 1 bug, 2 ideas")
    const summaryParts = Object.entries(typeCounts)
        .map(([type, count]) => {
            const plural = count > 1 ? 's' : '';
            return `${count} ${type}${plural}`;
        })
        .sort()
        .join(', ');
    
    // Update header
    const titleEl = panel.querySelector('#info-panel-task-title');
    titleEl.textContent = `Batch Selection (${selectedNodes.length} items)`;
    
    // Update ID section with summary
    const idEl = panel.querySelector('#info-panel-id');
    idEl.textContent = summaryParts;
    
    // Hide short name row
    const shortNameRow = panel.querySelector('#info-panel-short-name-row');
    shortNameRow.style.display = 'none';
    
    // Update meta
    const metaEl = panel.querySelector('#info-panel-meta');
    metaEl.textContent = `${selectedNodes.length} entities selected`;
    
    // Hide all detail sections
    panel.querySelector('#info-panel-queue-section').style.display = 'none';
    panel.querySelector('#info-panel-doc-open-section').style.display = 'none';
    panel.querySelector('#info-panel-summary-section').style.display = 'none';
    panel.querySelector('#info-panel-tags-section').style.display = 'none';
    panel.querySelector('#info-panel-assignee-section').style.display = 'none';
    panel.querySelector('#info-panel-deps-section').style.display = 'none';
    panel.querySelector('#info-panel-relationships-section').style.display = 'none';
    panel.querySelector('#info-panel-closed-section').style.display = 'none';
    panel.querySelector('#info-panel-timestamps-section').style.display = 'none';
    
    // Show description section with entity list
    const descSection = panel.querySelector('#info-panel-description-section');
    descSection.style.display = 'block';
    
    const descEl = panel.querySelector('#info-panel-description');
    descEl.innerHTML = '';
    descEl.classList.remove('empty');
    
    // Create a container for the entity list
    const listContainer = document.createElement('div');
    listContainer.className = 'batch-entity-list';
    
    // Add each selected entity with a checkbox
    selectedNodes.forEach(node => {
        const item = document.createElement('div');
        item.className = 'batch-entity-item';
        item.dataset.nodeId = node.id;
        
        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.className = 'batch-entity-checkbox';
        checkbox.dataset.nodeId = node.id;
        checkbox.addEventListener('change', (e) => {
            // Dispatch custom event for parent to handle
            panel.dispatchEvent(new CustomEvent('batch-item-toggle', {
                detail: { nodeId: node.id, selected: e.target.checked }
            }));
        });
        
        const idSpan = createClickableId(node.id);
        idSpan.className = 'batch-entity-id';
        
        const titleSpan = document.createElement('span');
        titleSpan.className = 'batch-entity-title';
        titleSpan.textContent = node.short_name || node.title || 'Untitled';
        
        const typeSpan = document.createElement('span');
        typeSpan.className = 'batch-entity-type';
        typeSpan.textContent = node.type || 'unknown';
        
        item.appendChild(checkbox);
        item.appendChild(idSpan);
        item.appendChild(titleSpan);
        item.appendChild(typeSpan);
        
        listContainer.appendChild(item);
    });
    
    descEl.appendChild(listContainer);
    
    // Add batch action buttons section
    const batchActionsSection = document.createElement('div');
    batchActionsSection.className = 'info-panel-section batch-actions-section';
    
    // If 2+ nodes selected, add "Create Link" button
    const createLinkBtn = selectedNodes.length >= 2 
        ? `<button class="batch-action-btn batch-action-btn-link" data-action="create-link">🔗 Create Link</button>`
        : '';
    
    batchActionsSection.innerHTML = `
        <div class="info-panel-section-title">Batch Actions</div>
        <div class="batch-actions-container">
            ${createLinkBtn}
            <button class="batch-action-btn" data-action="summarize">📊 Summarize</button>
            <button class="batch-action-btn" data-action="close">Close Selected</button>
            <button class="batch-action-btn" data-action="queue-add">Add to Queue</button>
            <button class="batch-action-btn" data-action="queue-remove">Remove from Queue</button>
            <button class="batch-action-btn" data-action="export">Export Selection</button>
        </div>
    `;
    
    // Add event listeners to batch action buttons
    const actionButtons = batchActionsSection.querySelectorAll('.batch-action-btn');
    actionButtons.forEach(btn => {
        btn.addEventListener('click', () => {
            const action = btn.dataset.action;
            panel.dispatchEvent(new CustomEvent('batch-action', {
                detail: { action, nodeIds: selectedNodes.map(n => n.id) }
            }));
        });
    });
    
    // Insert batch actions after description section
    descSection.parentNode.insertBefore(batchActionsSection, descSection.nextSibling);
    
    // Hide activity and commits tabs (not applicable for batch)
    panel.querySelectorAll('.info-panel-tab').forEach(tab => {
        if (tab.dataset.tab === 'activity' || tab.dataset.tab === 'commits') {
            tab.style.display = 'none';
        } else {
            tab.style.display = 'block';
        }
    });
    
    // Show the panel
    showInfoPanel(panel);
}

/**
 * Update info panel content for a node or batch selection
 * @param {HTMLElement} panel - The info panel element
 * @param {Object} node - Node data object (for single selection)
 * @param {Array} selectedNodes - Array of selected node objects (for multi-selection)
 */
export function updateInfoPanelContent(panel, node, selectedNodes = []) {
    // If multiple nodes are selected, show batch view
    if (selectedNodes && selectedNodes.length > 1) {
        updateBatchView(panel, selectedNodes);
        return;
    }
    
    if (!node) {
        hideInfoPanel(panel);
        return;
    }
    
    // Clean up any batch-specific elements from previous batch view
    const existingBatchActions = panel.querySelector('.batch-actions-section');
    if (existingBatchActions) {
        existingBatchActions.remove();
    }
    
    // Restore activity and commits tabs visibility
    panel.querySelectorAll('.info-panel-tab').forEach(tab => {
        tab.style.display = 'block';
    });
    
    // Store current node ID in panel data attribute for callbacks
    panel.dataset.currentNodeId = node.id || '';
    
    // Update ID (make it clickable)
    const idEl = panel.querySelector('#info-panel-id');
    idEl.textContent = '';
    idEl.appendChild(createClickableId(node.id || ''));
    
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
    
    // For agent nodes, show container ID if available, otherwise PID
    if (node.type === 'agent') {
        if (node.container_id) {
            metaParts.push(`Container: ${node.container_id}`);
        } else if (node.pid !== undefined && node.pid !== null) {
            metaParts.push(`PID: ${node.pid}`);
        }
    }
    
    metaEl.textContent = metaParts.join(' • ');
    
    // Update summary section (status, priority badges for expanded view)
    const summarySection = panel.querySelector('#info-panel-summary-section');
    const summaryContent = panel.querySelector('#info-panel-summary-content');
    
    // For doc nodes, make the summary section clickable to open the doc viewer
    if (node.type === 'doc') {
        summarySection.style.display = 'block';
        summarySection.classList.add('clickable');
        summarySection.title = 'Click to open document viewer';
        
        // Create a link-like appearance for the summary
        const docTypeLabel = node.doc_type ? `${node.doc_type.toUpperCase()}` : 'DOC';
        summaryContent.innerHTML = `<span class="doc-summary-link">📖 ${escapeHtml(docTypeLabel)}: Click to view full document</span>`;
    } else {
        // Remove clickable styling for non-doc nodes
        summarySection.classList.remove('clickable');
        summarySection.title = '';
        
        if (node.status || node.priority !== undefined) {
            summarySection.style.display = 'block';
            const summaryParts = [];
            if (node.status) summaryParts.push(`Status: ${node.status}`);
            if (node.priority !== undefined && node.priority !== null) summaryParts.push(`Priority: ${node.priority}`);
            summaryContent.textContent = summaryParts.join(' • ');
        } else {
            summarySection.style.display = 'none';
        }
    }
    
    // Update description
    const descEl = panel.querySelector('#info-panel-description');
    if (node.description) {
        descEl.textContent = node.description;
        descEl.classList.remove('empty');
    } else {
        descEl.textContent = 'No description provided.';
        descEl.classList.add('empty');
    }
    
    // Update tags (if any)
    const tagsSection = panel.querySelector('#info-panel-tags-section');
    const tagsEl = panel.querySelector('#info-panel-tags');
    if (node.tags && node.tags.length > 0) {
        tagsSection.style.display = 'block';
        tagsEl.innerHTML = node.tags.map(tag => 
            `<span class="tag-badge">${escapeHtml(tag)}</span>`
        ).join('');
    } else {
        tagsSection.style.display = 'none';
    }
    
    // Update assignee (if present)
    const assigneeSection = panel.querySelector('#info-panel-assignee-section');
    const assigneeEl = panel.querySelector('#info-panel-assignee');
    if (node.assignee) {
        assigneeSection.style.display = 'block';
        assigneeEl.textContent = node.assignee;
    } else {
        assigneeSection.style.display = 'none';
    }
    
    // Update dependencies (if any)
    const depsSection = panel.querySelector('#info-panel-deps-section');
    const depsEl = panel.querySelector('#info-panel-deps');
    if (node.depends_on && node.depends_on.length > 0) {
        depsSection.style.display = 'block';
        depsEl.innerHTML = '';
        node.depends_on.forEach(dep => {
            const li = document.createElement('li');
            li.appendChild(createClickableId(dep));
            depsEl.appendChild(li);
        });
    } else {
        depsSection.style.display = 'none';
    }
    
    // Update relationships (edges)
    const relationshipsSection = panel.querySelector('#info-panel-relationships-section');
    const relationshipsEl = panel.querySelector('#info-panel-relationships');
    if (node.edges && node.edges.length > 0) {
        relationshipsSection.style.display = 'block';
        relationshipsEl.innerHTML = '';
        node.edges.forEach(edge => {
            const li = document.createElement('li');
            li.className = 'relationship-item';
            li.dataset.nodeId = edge.related_id;
            
            const direction = edge.direction === 'outbound' ? '→' : '←';
            const edgeTypeFormatted = formatEdgeType(edge.edge_type);
            
            const dirSpan = document.createElement('span');
            dirSpan.className = 'relationship-direction';
            dirSpan.textContent = direction;
            
            const idSpan = createClickableId(edge.related_id);
            idSpan.className = 'relationship-id clickable-id';
            
            // Override the default click behavior to dispatch relationship-click event
            // Remove the default click listener and add our custom one
            const newIdSpan = idSpan.cloneNode(true); // Clone to remove listeners
            newIdSpan.addEventListener('click', (e) => {
                e.preventDefault();
                e.stopPropagation();
                // Dispatch custom event for relationship clicks
                panel.dispatchEvent(new CustomEvent('relationship-click', {
                    detail: { nodeId: edge.related_id }
                }));
            });
            
            const typeSpan = document.createElement('span');
            typeSpan.className = 'relationship-type';
            typeSpan.textContent = `(${edgeTypeFormatted})`;
            
            li.appendChild(dirSpan);
            li.appendChild(document.createTextNode(' '));
            li.appendChild(newIdSpan);
            li.appendChild(document.createTextNode(' '));
            li.appendChild(typeSpan);
            
            relationshipsEl.appendChild(li);
        });
    } else {
        relationshipsSection.style.display = 'none';
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
    
    // Update timestamps section
    const timestampsSection = panel.querySelector('#info-panel-timestamps-section');
    const timestampsEl = panel.querySelector('#info-panel-timestamps');
    if (node.created_at || node.updated_at || node.closed_at) {
        timestampsSection.style.display = 'block';
        let timestampsHTML = '';
        if (node.created_at) {
            timestampsHTML += `<dt>Created</dt><dd>${formatDate(node.created_at)}</dd>`;
        }
        if (node.updated_at) {
            timestampsHTML += `<dt>Updated</dt><dd>${formatDate(node.updated_at)}</dd>`;
        }
        if (node.closed_at) {
            timestampsHTML += `<dt>Closed</dt><dd>${formatDate(node.closed_at)}</dd>`;
        }
        timestampsEl.innerHTML = timestampsHTML;
    } else {
        timestampsSection.style.display = 'none';
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
        docSection.dataset.docId = node.id;
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
