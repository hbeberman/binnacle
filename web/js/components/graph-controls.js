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
 * Announce a message to screen readers
 * @param {string} message - The message to announce
 * @param {string} priority - 'polite' or 'assertive'
 */
function announceToScreenReader(message, priority = 'polite') {
    const announcement = document.createElement('div');
    announcement.setAttribute('role', 'status');
    announcement.setAttribute('aria-live', priority);
    announcement.setAttribute('aria-atomic', 'true');
    announcement.className = 'sr-only';
    announcement.textContent = message;
    document.body.appendChild(announcement);
    
    // Remove after announcement
    setTimeout(() => {
        document.body.removeChild(announcement);
    }, 1000);
}

/**
 * Add keyboard support to a button (Enter and Space)
 * @param {HTMLElement} button - The button element
 * @param {Function} callback - Function to call on activation
 */
function addKeyboardSupport(button, callback) {
    button.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            callback(e);
        }
    });
}

/**
 * Create graph controls HTML structure
 * @returns {HTMLElement} Graph controls container
 */
export function createGraphControls() {
    const controls = document.createElement('div');
    controls.className = 'graph-controls';
    controls.id = 'graph-controls';
    
    controls.innerHTML = `
        <input class="graph-search" id="graph-search" type="text" placeholder="Search nodes…" autocomplete="off" spellcheck="false" aria-label="Search nodes" />
        <div class="filters-dropdown" style="position: relative;">
            <button class="filters-dropdown-btn" id="filters-dropdown-btn" title="Show/hide filters" aria-expanded="false" aria-haspopup="true" aria-label="Show or hide visibility filters">
                <span class="filters-icon">⚙</span>
                <span class="filters-label">Filters</span>
            </button>
            <div class="filters-dropdown-popover" id="filters-dropdown-popover" role="menu" aria-label="Visibility filters">
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
            <span class="hide-completed-label" id="hide-completed-label">Hide completed</span>
            <button class="hide-completed-switch" id="hide-completed-switch" title="Hide completed nodes (except in active chains)" role="switch" aria-checked="false" aria-labelledby="hide-completed-label"></button>
        </div>
        <div class="auto-follow-toggle" style="position: relative;">
            <span class="auto-follow-label" id="auto-follow-label">Follow Agents</span>
            <button class="auto-follow-switch" id="auto-follow-switch" title="Toggle follow agents mode" role="switch" aria-checked="false" aria-labelledby="auto-follow-label"></button>
            <button class="auto-follow-config-btn" id="auto-follow-config-btn" title="Configure auto-follow settings" aria-expanded="false" aria-haspopup="true" aria-label="Configure auto-follow settings">⚙</button>
            <div class="auto-follow-config-popover" id="auto-follow-config-popover" role="menu" aria-label="Auto-follow settings">
                <div class="config-popover-title">Auto-Follow Settings</div>
                <div class="config-section">
                    <span class="config-section-label">Auto-follow for new:</span>
                    <div class="config-node-types">
                        <div class="config-node-type-row">
                            <span class="config-node-type-label" id="config-follow-tasks-label"><span class="config-node-type-icon" aria-hidden="true">📋</span> Tasks</span>
                            <button class="config-toggle active" id="config-follow-tasks" data-type="task" role="switch" aria-checked="true" aria-labelledby="config-follow-tasks-label"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label" id="config-follow-bugs-label"><span class="config-node-type-icon" aria-hidden="true">🐛</span> Bugs</span>
                            <button class="config-toggle active" id="config-follow-bugs" data-type="bug" role="switch" aria-checked="true" aria-labelledby="config-follow-bugs-label"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label" id="config-follow-ideas-label"><span class="config-node-type-icon" aria-hidden="true">💡</span> Ideas</span>
                            <button class="config-toggle" id="config-follow-ideas" data-type="idea" role="switch" aria-checked="false" aria-labelledby="config-follow-ideas-label"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label" id="config-follow-tests-label"><span class="config-node-type-icon" aria-hidden="true">🧪</span> Tests</span>
                            <button class="config-toggle" id="config-follow-tests" data-type="test" role="switch" aria-checked="false" aria-labelledby="config-follow-tests-label"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label" id="config-follow-docs-label"><span class="config-node-type-icon" aria-hidden="true">📄</span> Docs</span>
                            <button class="config-toggle" id="config-follow-docs" data-type="doc" role="switch" aria-checked="false" aria-labelledby="config-follow-docs-label"></button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
        <div class="follow-events-toggle" style="position: relative;">
            <span class="follow-events-label" id="follow-events-label">Follow Events</span>
            <button class="follow-events-switch" id="follow-events-switch" title="Toggle follow events mode" role="switch" aria-checked="false" aria-labelledby="follow-events-label"></button>
            <button class="follow-events-config-btn" id="follow-events-config-btn" title="Configure follow events settings" aria-expanded="false" aria-haspopup="true" aria-label="Configure follow events settings">⚙</button>
            <div class="follow-events-config-popover" id="follow-events-config-popover" role="menu" aria-label="Follow events settings">
                <div class="config-popover-title">Follow Events Settings</div>
                <div class="config-section">
                    <span class="config-section-label">Follow new:</span>
                    <div class="config-node-types">
                        <div class="config-node-type-row">
                            <span class="config-node-type-label" id="config-events-tasks-label"><span class="config-node-type-icon" aria-hidden="true">📋</span> Tasks</span>
                            <button class="config-toggle active" id="config-events-tasks" data-type="task" role="switch" aria-checked="true" aria-labelledby="config-events-tasks-label"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label" id="config-events-bugs-label"><span class="config-node-type-icon" aria-hidden="true">🐛</span> Bugs</span>
                            <button class="config-toggle active" id="config-events-bugs" data-type="bug" role="switch" aria-checked="true" aria-labelledby="config-events-bugs-label"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label" id="config-events-ideas-label"><span class="config-node-type-icon" aria-hidden="true">💡</span> Ideas</span>
                            <button class="config-toggle active" id="config-events-ideas" data-type="idea" role="switch" aria-checked="true" aria-labelledby="config-events-ideas-label"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label" id="config-events-tests-label"><span class="config-node-type-icon" aria-hidden="true">🧪</span> Tests</span>
                            <button class="config-toggle active" id="config-events-tests" data-type="test" role="switch" aria-checked="true" aria-labelledby="config-events-tests-label"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label" id="config-events-docs-label"><span class="config-node-type-icon" aria-hidden="true">📄</span> Docs</span>
                            <button class="config-toggle active" id="config-events-docs" data-type="doc" role="switch" aria-checked="true" aria-labelledby="config-events-docs-label"></button>
                        </div>
                        <div class="config-node-type-row">
                            <span class="config-node-type-label" id="config-events-milestones-label"><span class="config-node-type-icon" aria-hidden="true">🎯</span> Milestones</span>
                            <button class="config-toggle active" id="config-events-milestones" data-type="milestone" role="switch" aria-checked="true" aria-labelledby="config-events-milestones-label"></button>
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
        const toggleFilters = (e) => {
            e.stopPropagation();
            const isExpanded = filtersBtn.classList.toggle('active');
            filtersPopover.classList.toggle('visible');
            filtersBtn.setAttribute('aria-expanded', isExpanded);
            
            if (isExpanded) {
                announceToScreenReader('Filters menu opened');
            } else {
                announceToScreenReader('Filters menu closed');
            }
        };
        
        filtersBtn.addEventListener('click', toggleFilters);
        addKeyboardSupport(filtersBtn, toggleFilters);
        
        // Close popover when clicking outside
        document.addEventListener('click', (e) => {
            if (!filtersPopover.contains(e.target) && e.target !== filtersBtn) {
                filtersBtn.classList.remove('active');
                filtersPopover.classList.remove('visible');
                filtersBtn.setAttribute('aria-expanded', 'false');
            }
        });
        
        // Close on Escape key
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Escape' && filtersPopover.classList.contains('visible')) {
                filtersBtn.classList.remove('active');
                filtersPopover.classList.remove('visible');
                filtersBtn.setAttribute('aria-expanded', 'false');
                filtersBtn.focus();
                announceToScreenReader('Filters menu closed');
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
                State.set('ui.searchMatches', []);
                State.set('ui.currentMatchIndex', -1);
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
        hideCompletedSwitch.setAttribute('aria-checked', hideCompleted);
        
        const toggleHideCompleted = () => {
            const newState = !hideCompletedSwitch.classList.contains('active');
            hideCompletedSwitch.classList.toggle('active', newState);
            hideCompletedSwitch.setAttribute('aria-checked', newState);
            State.set('ui.hideCompleted', newState);
            
            announceToScreenReader(`Hide completed nodes ${newState ? 'enabled' : 'disabled'}`);
            
            if (onHideCompletedToggle) {
                onHideCompletedToggle(newState);
            }
        };
        
        hideCompletedSwitch.addEventListener('click', toggleHideCompleted);
        addKeyboardSupport(hideCompletedSwitch, toggleHideCompleted);
    }
    
    // Initialize follow agents toggle switch
    const autoFollowSwitch = controls.querySelector('#auto-follow-switch');
    if (autoFollowSwitch) {
        // Load initial state (default to enabled - follow agents mode)
        const storedFollowType = State.get('ui.followTypeFilter');
        const followType = storedFollowType !== null && storedFollowType !== undefined ? storedFollowType : 'agent';
        
        // Toggle is active when following agents
        const isFollowingAgents = followType === 'agent';
        autoFollowSwitch.classList.toggle('active', isFollowingAgents);
        autoFollowSwitch.setAttribute('aria-checked', isFollowingAgents);
        
        // Set initial state
        State.set('ui.followTypeFilter', isFollowingAgents ? 'agent' : 'none');
        State.set('ui.autoFollow', isFollowingAgents);
        
        const toggleAutoFollow = () => {
            const newState = !autoFollowSwitch.classList.contains('active');
            autoFollowSwitch.classList.toggle('active', newState);
            autoFollowSwitch.setAttribute('aria-checked', newState);
            
            // Set follow type: 'agent' when on, 'none' when off
            const newType = newState ? 'agent' : 'none';
            State.set('ui.followTypeFilter', newType);
            State.set('ui.autoFollow', newState);
            
            announceToScreenReader(`Follow agents mode ${newState ? 'enabled' : 'disabled'}`);
            
            // Trigger the auto-follow callback with the new state
            if (onAutoFollowToggle) {
                onAutoFollowToggle(newState);
            }
            
            // Optionally trigger type change callback
            if (options.onFollowTypeChange) {
                options.onFollowTypeChange(newType);
            }
        };
        
        autoFollowSwitch.addEventListener('click', toggleAutoFollow);
        addKeyboardSupport(autoFollowSwitch, toggleAutoFollow);
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
                    toggle.setAttribute('aria-checked', enabled);
                }
            });
        };
        
        updateConfigToggles();
        
        // Toggle popover visibility
        const toggleConfigPopover = (e) => {
            e.stopPropagation();
            const isExpanded = configBtn.classList.toggle('active');
            configPopover.classList.toggle('visible');
            configBtn.setAttribute('aria-expanded', isExpanded);
            
            if (isExpanded) {
                announceToScreenReader('Auto-follow settings menu opened');
            } else {
                announceToScreenReader('Auto-follow settings menu closed');
            }
        };
        
        configBtn.addEventListener('click', toggleConfigPopover);
        addKeyboardSupport(configBtn, toggleConfigPopover);
        
        // Close popover when clicking outside
        document.addEventListener('click', (e) => {
            if (!configPopover.contains(e.target) && e.target !== configBtn) {
                configBtn.classList.remove('active');
                configPopover.classList.remove('visible');
                configBtn.setAttribute('aria-expanded', 'false');
            }
        });
        
        // Close on Escape key
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Escape' && configPopover.classList.contains('visible')) {
                configBtn.classList.remove('active');
                configPopover.classList.remove('visible');
                configBtn.setAttribute('aria-expanded', 'false');
                configBtn.focus();
                announceToScreenReader('Auto-follow settings menu closed');
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
                        toggle.setAttribute('aria-checked', config.nodeTypes[type]);
                        
                        const typeName = type.charAt(0).toUpperCase() + type.slice(1);
                        announceToScreenReader(`Auto-follow for ${typeName}s ${config.nodeTypes[type] ? 'enabled' : 'disabled'}`);
                    }
                }
            }
        });
        
        // Add keyboard support to config toggles
        configPopover.querySelectorAll('.config-toggle').forEach(toggle => {
            addKeyboardSupport(toggle, () => {
                toggle.click();
            });
        });
    }
    
    // Initialize follow events toggle switch
    const followEventsSwitch = controls.querySelector('#follow-events-switch');
    if (followEventsSwitch) {
        // Load initial state (default to disabled)
        const followEvents = State.get('ui.followEvents') || false;
        followEventsSwitch.classList.toggle('active', followEvents);
        followEventsSwitch.setAttribute('aria-checked', followEvents);
        
        const toggleFollowEvents = () => {
            const newState = !followEventsSwitch.classList.contains('active');
            followEventsSwitch.classList.toggle('active', newState);
            followEventsSwitch.setAttribute('aria-checked', newState);
            State.set('ui.followEvents', newState);
            
            announceToScreenReader(`Follow events mode ${newState ? 'enabled' : 'disabled'}`);
            
            // Trigger callback if provided
            if (options.onFollowEventsToggle) {
                options.onFollowEventsToggle(newState);
            }
        };
        
        followEventsSwitch.addEventListener('click', toggleFollowEvents);
        addKeyboardSupport(followEventsSwitch, toggleFollowEvents);
    }
    
    // Initialize follow events config button
    const eventsConfigBtn = controls.querySelector('#follow-events-config-btn');
    const eventsConfigPopover = controls.querySelector('#follow-events-config-popover');
    
    if (eventsConfigBtn && eventsConfigPopover) {
        // Load follow events config from state
        let followEventsConfig = State.get('ui.followEventsConfig');
        if (!followEventsConfig) {
            followEventsConfig = {
                nodeTypes: {
                    task: true,
                    bug: true,
                    idea: true,
                    test: true,
                    doc: true,
                    milestone: true
                }
            };
            State.set('ui.followEventsConfig', followEventsConfig);
        }
        
        // Update config toggle states
        const updateEventsConfigToggles = () => {
            const config = State.get('ui.followEventsConfig');
            if (!config) return;
            
            Object.entries(config.nodeTypes).forEach(([type, enabled]) => {
                const toggle = eventsConfigPopover.querySelector(`#config-events-${type}s`);
                if (toggle) {
                    toggle.classList.toggle('active', enabled);
                    toggle.setAttribute('aria-checked', enabled);
                }
            });
        };
        
        updateEventsConfigToggles();
        
        // Toggle popover visibility
        const toggleEventsConfigPopover = (e) => {
            e.stopPropagation();
            const isExpanded = eventsConfigBtn.classList.toggle('active');
            eventsConfigPopover.classList.toggle('visible');
            eventsConfigBtn.setAttribute('aria-expanded', isExpanded);
            
            if (isExpanded) {
                announceToScreenReader('Follow events settings menu opened');
            } else {
                announceToScreenReader('Follow events settings menu closed');
            }
        };
        
        eventsConfigBtn.addEventListener('click', toggleEventsConfigPopover);
        addKeyboardSupport(eventsConfigBtn, toggleEventsConfigPopover);
        
        // Close popover when clicking outside
        document.addEventListener('click', (e) => {
            if (!eventsConfigPopover.contains(e.target) && e.target !== eventsConfigBtn) {
                eventsConfigBtn.classList.remove('active');
                eventsConfigPopover.classList.remove('visible');
                eventsConfigBtn.setAttribute('aria-expanded', 'false');
            }
        });
        
        // Close on Escape key
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Escape' && eventsConfigPopover.classList.contains('visible')) {
                eventsConfigBtn.classList.remove('active');
                eventsConfigPopover.classList.remove('visible');
                eventsConfigBtn.setAttribute('aria-expanded', 'false');
                eventsConfigBtn.focus();
                announceToScreenReader('Follow events settings menu closed');
            }
        });
        
        // Handle config toggle clicks
        eventsConfigPopover.addEventListener('click', (e) => {
            const toggle = e.target.closest('.config-toggle');
            if (toggle) {
                const type = toggle.dataset.type;
                if (type) {
                    const config = State.get('ui.followEventsConfig');
                    if (Object.prototype.hasOwnProperty.call(config.nodeTypes, type)) {
                        config.nodeTypes[type] = !config.nodeTypes[type];
                        State.set('ui.followEventsConfig', config);
                        toggle.classList.toggle('active', config.nodeTypes[type]);
                        toggle.setAttribute('aria-checked', config.nodeTypes[type]);
                        
                        const typeName = type.charAt(0).toUpperCase() + type.slice(1);
                        announceToScreenReader(`Follow events for ${typeName}s ${config.nodeTypes[type] ? 'enabled' : 'disabled'}`);
                    }
                }
            }
        });
        
        // Add keyboard support to config toggles
        eventsConfigPopover.querySelectorAll('.config-toggle').forEach(toggle => {
            addKeyboardSupport(toggle, () => {
                toggle.click();
            });
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
                
                // Click to focus agent in graph and pin follow to that agent
                item.addEventListener('click', () => {
                    State.set('ui.selectedNode', agent.id);
                    State.set('ui.followTypeFilter', 'agent');
                    State.set('ui.autoFollow', true);
                    State.set('ui.pinnedAgentId', agent.id);  // Pin to this specific agent
                    
                    // Update the toggle switch UI
                    const autoFollowSwitch = controls.querySelector('#auto-follow-switch');
                    if (autoFollowSwitch) {
                        autoFollowSwitch.classList.add('active');
                    }
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
