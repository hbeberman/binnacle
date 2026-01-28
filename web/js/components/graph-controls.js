/**
 * Graph Controls Component
 * 
 * Overlay controls for the graph view:
 * - Search input for filtering nodes
 * - Hide completed toggle
 * - Auto-follow configuration
 * - Agent dropdown menu
 * - Zoom controls
 */

import * as State from '../state.js';

/**
 * Create graph controls HTML structure
 * @returns {HTMLElement} Graph controls container
 */
export function createGraphControls() {
    const controls = document.createElement('div');
    controls.className = 'graph-controls';
    controls.id = 'graph-controls';
    
    controls.innerHTML = `
        <input class="graph-search" id="graph-search" type="text" placeholder="Search nodes…" autocomplete="off" spellcheck="false" />
        <div class="filters-dropdown" style="position: relative;">
            <button class="filters-dropdown-btn" id="filters-dropdown-btn" title="Show/hide filters">
                <span class="filters-icon">⚙</span>
                <span class="filters-label">Filters</span>
            </button>
            <div class="filters-dropdown-popover" id="filters-dropdown-popover">
                <div class="config-popover-title">Visibility Filters</div>
                <div class="config-section">
                    <span class="config-section-label">Nodes</span>
                    <div class="sidebar-filter-group" id="graph-node-filters">
                        <!-- Node type filter buttons will be populated dynamically -->
                    </div>
                </div>
                <div class="config-section">
                    <span class="config-section-label">Edges</span>
                    <div class="sidebar-filter-group" id="graph-edge-filters">
                        <!-- Edge type filter buttons will be populated dynamically -->
                    </div>
                </div>
            </div>
        </div>
        <div class="hide-completed-toggle">
            <span class="hide-completed-label">Hide completed</span>
            <button class="hide-completed-switch" id="hide-completed-switch" title="Hide completed nodes (except in active chains)"></button>
        </div>
        <div class="auto-follow-toggle" style="position: relative;">
            <span class="auto-follow-label">Follow</span>
            <select class="follow-type-selector" id="follow-type-selector" title="Select type to follow">
                <option value="none">None</option>
                <option value="">Any</option>
                <option value="task">📋 Tasks</option>
                <option value="bug">🐛 Bugs</option>
                <option value="idea">💡 Ideas</option>
                <option value="agent">🤖 Agents</option>
            </select>
            <button class="auto-follow-config-btn" id="auto-follow-config-btn" title="Configure auto-follow settings">⚙</button>
            <div class="auto-follow-config-popover" id="auto-follow-config-popover">
                <div class="config-popover-title">Auto-Follow Settings</div>
                <div class="config-section">
                    <span class="config-section-label">Auto-follow for new:</span>
                    <div class="config-node-types">
                        <div class="config-node-type-row">
                            <span class="config-node-type-label"><span class="config-node-type-icon">📋</span> Tasks</span>
                            <button class="config-toggle active" id="config-follow-tasks" data-type="task"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label"><span class="config-node-type-icon">🐛</span> Bugs</span>
                            <button class="config-toggle active" id="config-follow-bugs" data-type="bug"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label"><span class="config-node-type-icon">💡</span> Ideas</span>
                            <button class="config-toggle" id="config-follow-ideas" data-type="idea"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label"><span class="config-node-type-icon">🧪</span> Tests</span>
                            <button class="config-toggle" id="config-follow-tests" data-type="test"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label"><span class="config-node-type-icon">📄</span> Docs</span>
                            <button class="config-toggle" id="config-follow-docs" data-type="doc"></button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
        <div class="graph-controls-spacer"></div>
        <div class="zoom-controls-inline">
            <button class="graph-control-btn zoom-out-btn" id="zoom-out-btn" title="Zoom Out">−</button>
            <div class="zoom-level" id="zoom-level">100%</div>
            <button class="graph-control-btn zoom-in-btn" id="zoom-in-btn" title="Zoom In">+</button>
        </div>
        <div class="agent-dropdown" style="position: relative;">
            <button class="agent-dropdown-btn" id="agent-dropdown-btn" title="Show agents">
                <span class="agent-icon">🤖</span>
                <span class="agent-label">Agents</span>
                <span class="agent-count" id="agent-count">0</span>
            </button>
            <div class="agent-dropdown-popover" id="agent-dropdown-popover">
                <div class="config-popover-title">Active Agents</div>
                <div class="agent-list" id="graph-agent-list">
                    <!-- Agent items will be populated dynamically -->
                </div>
            </div>
        </div>
    `;
    
    return controls;
}

/**
 * Initialize graph controls functionality
 * @param {HTMLElement} controls - The graph controls element
 * @param {Object} options - Configuration options
 * @param {Function} options.onSearch - Callback for search input changes
 * @param {Function} options.onHideCompletedToggle - Callback for hide completed toggle
 * @param {Function} options.onAutoFollowToggle - Callback for auto-follow toggle
 * @param {Function} options.onZoomIn - Callback for zoom in
 * @param {Function} options.onZoomOut - Callback for zoom out
 * @param {Function} options.updateZoomLevel - Callback to update zoom level display
 */
export function initializeGraphControls(controls, options = {}) {
    const {
        onSearch = null,
        onHideCompletedToggle = null,
        onAutoFollowToggle = null,
        onZoomIn = null,
        onZoomOut = null,
        updateZoomLevel = null
    } = options;
    
    // Initialize filters dropdown
    const filtersBtn = controls.querySelector('#filters-dropdown-btn');
    const filtersPopover = controls.querySelector('#filters-dropdown-popover');
    
    if (filtersBtn && filtersPopover) {
        // Toggle popover visibility
        filtersBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            filtersBtn.classList.toggle('active');
            filtersPopover.classList.toggle('visible');
        });
        
        // Close popover when clicking outside
        document.addEventListener('click', (e) => {
            if (!filtersPopover.contains(e.target) && e.target !== filtersBtn) {
                filtersBtn.classList.remove('active');
                filtersPopover.classList.remove('visible');
            }
        });
        
        // Initialize node and edge filters inside the popover
        import('./filters.js').then(({ initializeNodeTypeFilters, initializeEdgeTypeFilters }) => {
            initializeNodeTypeFilters('graph-node-filters');
            initializeEdgeTypeFilters('graph-edge-filters');
        }).catch(err => {
            console.error('Failed to load filter components:', err);
        });
    }
    
    // Initialize search input
    const searchInput = controls.querySelector('#graph-search');
    if (searchInput) {
        searchInput.addEventListener('input', () => {
            const query = searchInput.value.trim();
            State.set('ui.searchQuery', query);
            if (onSearch) {
                onSearch(query);
            }
        });
        
        searchInput.addEventListener('keydown', (e) => {
            if (e.key === 'Escape') {
                searchInput.value = '';
                State.set('ui.searchQuery', '');
                if (onSearch) {
                    onSearch('');
                }
                e.preventDefault();
            }
        });
    }
    
    // Initialize hide completed toggle
    const hideCompletedSwitch = controls.querySelector('#hide-completed-switch');
    if (hideCompletedSwitch) {
        // Load initial state
        const hideCompleted = State.get('ui.hideCompleted') !== false; // Default true
        hideCompletedSwitch.classList.toggle('active', hideCompleted);
        
        hideCompletedSwitch.addEventListener('click', () => {
            const newState = !hideCompletedSwitch.classList.contains('active');
            hideCompletedSwitch.classList.toggle('active', newState);
            State.set('ui.hideCompleted', newState);
            if (onHideCompletedToggle) {
                onHideCompletedToggle(newState);
            }
        });
    }
    
    // Initialize follow type selector with implicit auto-follow
    const followTypeSelector = controls.querySelector('#follow-type-selector');
    if (followTypeSelector) {
        // Load initial state (default to 'agent' to match "follow agents" requirement)
        const storedFollowType = State.get('ui.followTypeFilter');
        const followType = storedFollowType !== null && storedFollowType !== undefined ? storedFollowType : 'agent';
        followTypeSelector.value = followType;
        
        // Set initial auto-follow state based on selection
        const autoFollow = followType !== 'none';
        State.set('ui.autoFollow', autoFollow);
        
        followTypeSelector.addEventListener('change', () => {
            const newType = followTypeSelector.value;
            State.set('ui.followTypeFilter', newType);
            
            // Implicitly enable/disable auto-follow based on selection
            const shouldFollow = newType !== 'none';
            State.set('ui.autoFollow', shouldFollow);
            
            // Trigger the auto-follow callback with the new state
            if (onAutoFollowToggle) {
                onAutoFollowToggle(shouldFollow);
            }
            
            // Optionally trigger type change callback
            if (options.onFollowTypeChange) {
                options.onFollowTypeChange(newType);
            }
        });
    }
    
    // Initialize auto-follow config button
    const configBtn = controls.querySelector('#auto-follow-config-btn');
    const configPopover = controls.querySelector('#auto-follow-config-popover');
    
    if (configBtn && configPopover) {
        // Load auto-follow config from state
        let autoFollowConfig = State.get('ui.autoFollowConfig');
        if (!autoFollowConfig) {
            autoFollowConfig = {
                nodeTypes: {
                    task: true,
                    bug: true,
                    idea: false,
                    test: false,
                    doc: false
                }
            };
            State.set('ui.autoFollowConfig', autoFollowConfig);
        }
        
        // Update config toggle states
        const updateConfigToggles = () => {
            const config = State.get('ui.autoFollowConfig');
            if (!config) return;
            
            Object.entries(config.nodeTypes).forEach(([type, enabled]) => {
                const toggle = configPopover.querySelector(`#config-follow-${type}s`);
                if (toggle) {
                    toggle.classList.toggle('active', enabled);
                }
            });
        };
        
        updateConfigToggles();
        
        // Toggle popover visibility
        configBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            configBtn.classList.toggle('active');
            configPopover.classList.toggle('visible');
        });
        
        // Close popover when clicking outside
        document.addEventListener('click', (e) => {
            if (!configPopover.contains(e.target) && e.target !== configBtn) {
                configBtn.classList.remove('active');
                configPopover.classList.remove('visible');
            }
        });
        
        // Handle config toggle clicks
        configPopover.addEventListener('click', (e) => {
            const toggle = e.target.closest('.config-toggle');
            if (toggle) {
                const type = toggle.dataset.type;
                if (type) {
                    const config = State.get('ui.autoFollowConfig');
                    if (Object.prototype.hasOwnProperty.call(config.nodeTypes, type)) {
                        config.nodeTypes[type] = !config.nodeTypes[type];
                        State.set('ui.autoFollowConfig', config);
                        toggle.classList.toggle('active', config.nodeTypes[type]);
                    }
                }
            }
        });
    }
    
    // Initialize zoom controls
    const zoomInBtn = controls.querySelector('#zoom-in-btn');
    const zoomOutBtn = controls.querySelector('#zoom-out-btn');
    const zoomLevel = controls.querySelector('#zoom-level');
    
    if (zoomInBtn && onZoomIn) {
        zoomInBtn.addEventListener('click', () => {
            onZoomIn();
            if (updateZoomLevel && zoomLevel) {
                updateZoomLevel(zoomLevel);
            }
        });
    }
    
    if (zoomOutBtn && onZoomOut) {
        zoomOutBtn.addEventListener('click', () => {
            onZoomOut();
            if (updateZoomLevel && zoomLevel) {
                updateZoomLevel(zoomLevel);
            }
        });
    }
    
    // Initial zoom level update
    if (updateZoomLevel && zoomLevel) {
        updateZoomLevel(zoomLevel);
    }
    
    // Initialize agent dropdown
    const agentBtn = controls.querySelector('#agent-dropdown-btn');
    const agentPopover = controls.querySelector('#agent-dropdown-popover');
    const agentList = controls.querySelector('#graph-agent-list');
    const agentCount = controls.querySelector('#agent-count');
    
    if (agentBtn && agentPopover) {
        // Toggle popover visibility
        agentBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            agentBtn.classList.toggle('active');
            agentPopover.classList.toggle('visible');
        });
        
        // Close popover when clicking outside
        document.addEventListener('click', (e) => {
            if (!agentPopover.contains(e.target) && e.target !== agentBtn) {
                agentBtn.classList.remove('active');
                agentPopover.classList.remove('visible');
            }
        });
        
        // Update agent list from state
        const updateAgentList = () => {
            if (!agentList) return;
            
            const agents = State.get('entities.agents') || [];
            
            // Update count badge
            if (agentCount) {
                agentCount.textContent = agents.length;
                agentCount.style.display = agents.length > 0 ? 'inline-block' : 'none';
            }
            
            // Clear existing items
            agentList.innerHTML = '';
            
            if (agents.length === 0) {
                const emptyState = document.createElement('div');
                emptyState.className = 'agent-list-empty';
                emptyState.textContent = 'No agents';
                agentList.appendChild(emptyState);
                return;
            }
            
            // Sort agents: active first, then idle, then stale
            const statusOrder = { 'active': 0, 'idle': 1, 'stale': 2, 'unknown': 3 };
            const sortedAgents = [...agents].sort((a, b) => {
                const aStatus = (a.status || 'unknown').toLowerCase();
                const bStatus = (b.status || 'unknown').toLowerCase();
                const orderA = statusOrder[aStatus] ?? 3;
                const orderB = statusOrder[bStatus] ?? 3;
                
                if (orderA !== orderB) {
                    return orderA - orderB;
                }
                
                // Secondary sort by name
                const nameA = a._agent?.name || a.title || a.id;
                const nameB = b._agent?.name || b.title || b.id;
                return nameA.localeCompare(nameB);
            });
            
            // Create agent items
            for (const agent of sortedAgents) {
                const item = document.createElement('div');
                item.className = 'agent-item';
                item.dataset.agentId = agent.id;
                
                // Get status badge
                const statusLower = (agent.status || 'unknown').toLowerCase();
                let badge;
                switch (statusLower) {
                    case 'active':
                        badge = { emoji: '🟢', className: 'status-active', label: 'Active' };
                        break;
                    case 'idle':
                        badge = { emoji: '🟡', className: 'status-idle', label: 'Idle' };
                        break;
                    case 'stale':
                        badge = { emoji: '🔴', className: 'status-stale', label: 'Stale' };
                        break;
                    default:
                        badge = { emoji: '⚪', className: 'status-unknown', label: 'Unknown' };
                }
                
                const name = agent._agent?.purpose || agent._agent?.name || agent.title || agent.id;
                
                item.innerHTML = `
                    <div class="agent-item-status ${badge.className}" title="${badge.label}">
                        ${badge.emoji}
                    </div>
                    <div class="agent-item-content">
                        <div class="agent-item-name">${escapeHtml(name)}</div>
                    </div>
                `;
                
                // Click to focus agent in graph and enable agent follow mode
                item.addEventListener('click', () => {
                    State.set('ui.selectedNode', agent.id);
                    State.set('ui.followTypeFilter', 'agent');
                    State.set('ui.autoFollow', true);
                });
                
                agentList.appendChild(item);
            }
        };
        
        // Subscribe to agent updates
        State.subscribe('entities.agents', updateAgentList);
        
        // Initial update
        updateAgentList();
    }
}

/**
 * Simple HTML escaping
 * @param {string} str - String to escape
 * @returns {string} Escaped string
 */
function escapeHtml(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

/**
 * Mount graph controls to a container
 * @param {HTMLElement} container - Container to mount controls to
 * @param {Object} options - Configuration options (passed to initializeGraphControls)
 * @returns {HTMLElement} The created controls element
 */
export function mountGraphControls(container, options = {}) {
    const controls = createGraphControls();
    container.appendChild(controls);
    initializeGraphControls(controls, options);
    return controls;
}
