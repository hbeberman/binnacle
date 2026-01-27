/**
 * Link Builder Component
 * 
 * Displays a UI for creating links between exactly 2 selected entities with:
 * - Link type dropdown
 * - Direction swap button
 * - Optional reason field
 * - Preview and create link buttons
 */

import { createClickableId } from '../utils/clickable-ids.js';

// Link type definitions with their descriptions
const LINK_TYPES = [
    { value: 'depends_on', label: 'Depends On', description: 'Source depends on target' },
    { value: 'blocks', label: 'Blocks', description: 'Source blocks target' },
    { value: 'child_of', label: 'Child Of', description: 'Source is child of target' },
    { value: 'parent_of', label: 'Parent Of', description: 'Source is parent of target' },
    { value: 'related_to', label: 'Related To', description: 'Source is related to target' },
    { value: 'tests', label: 'Tests', description: 'Source tests target' },
    { value: 'tested_by', label: 'Tested By', description: 'Source is tested by target' },
    { value: 'documents', label: 'Documents', description: 'Source documents target' },
    { value: 'documented_by', label: 'Documented By', description: 'Source is documented by target' },
    { value: 'queued', label: 'Queued', description: 'Source is queued in target' },
    { value: 'working_on', label: 'Working On', description: 'Source is working on target' },
    { value: 'informational', label: 'Informational', description: 'Informational link' }
];

/**
 * Create link builder HTML structure
 * @returns {HTMLElement} Link builder container element
 */
export function createLinkBuilder() {
    const builder = document.createElement('div');
    builder.className = 'link-builder';
    builder.id = 'link-builder';
    
    builder.innerHTML = `
        <div class="link-builder-header">
            <span class="link-builder-title">Create Relationship</span>
        </div>
        <div class="link-builder-content">
            <div class="link-builder-nodes">
                <div class="link-builder-node" id="link-builder-source">
                    <span class="link-builder-node-label">Source:</span>
                    <div class="link-builder-node-info"></div>
                </div>
                <button class="link-builder-swap" id="link-builder-swap" title="Swap source and target">
                    ⇅
                </button>
                <div class="link-builder-node" id="link-builder-target">
                    <span class="link-builder-node-label">Target:</span>
                    <div class="link-builder-node-info"></div>
                </div>
            </div>
            
            <div class="link-builder-multi-nodes" id="link-builder-multi-nodes" style="display: none;">
                <div class="link-builder-label">Selected Entities (<span id="link-builder-node-count">0</span>):</div>
                <div class="link-builder-node-list" id="link-builder-node-list"></div>
            </div>
            
            <div class="link-builder-mode" id="link-builder-mode" style="display: none;">
                <label class="link-builder-label">Linking Mode:</label>
                <div class="link-builder-mode-options">
                    <button class="link-builder-mode-btn active" data-mode="chain" title="Chain: A→B→C→D">
                        <span class="mode-icon">🔗</span> Chain
                    </button>
                    <button class="link-builder-mode-btn" data-mode="hub" title="Hub: All point to one">
                        <span class="mode-icon">⭐</span> Hub
                    </button>
                    <button class="link-builder-mode-btn" data-mode="mesh" title="Mesh: All connected to all">
                        <span class="mode-icon">🕸️</span> Mesh
                    </button>
                </div>
                <div class="link-builder-mode-description" id="link-builder-mode-description">
                    Chain: Creates sequential links A→B→C→D
                </div>
            </div>
            
            <div class="link-builder-type">
                <label for="link-builder-type-select" class="link-builder-label">Link Type:</label>
                <select id="link-builder-type-select" class="link-builder-select">
                    ${LINK_TYPES.map(type => `
                        <option value="${type.value}">${type.label}</option>
                    `).join('')}
                </select>
                <div class="link-builder-type-description" id="link-builder-type-description">
                    ${LINK_TYPES[0].description}
                </div>
            </div>
            
            <div class="link-builder-reason">
                <label for="link-builder-reason-input" class="link-builder-label">
                    Reason (optional):
                </label>
                <textarea 
                    id="link-builder-reason-input" 
                    class="link-builder-textarea"
                    placeholder="Why are these entities linked?"
                    rows="2"
                ></textarea>
            </div>
            
            <div class="link-builder-preview" id="link-builder-preview">
                <div class="link-builder-preview-title">Preview:</div>
                <div class="link-builder-preview-text" id="link-builder-preview-text"></div>
            </div>
            
            <div class="link-builder-actions">
                <button class="link-builder-btn link-builder-btn-cancel" id="link-builder-cancel">
                    Cancel
                </button>
                <button class="link-builder-btn link-builder-btn-create" id="link-builder-create">
                    Create Link
                </button>
            </div>
        </div>
    `;
    
    return builder;
}

/**
 * Initialize link builder event handlers
 * @param {HTMLElement} builder - The link builder element
 * @param {Object} callbacks - Callback functions
 * @param {Function} callbacks.onCreate - Called when link is created
 * @param {Function} callbacks.onCancel - Called when cancelled
 */
export function initializeLinkBuilder(builder, callbacks = {}) {
    const swapBtn = builder.querySelector('#link-builder-swap');
    const typeSelect = builder.querySelector('#link-builder-type-select');
    const reasonInput = builder.querySelector('#link-builder-reason-input');
    const cancelBtn = builder.querySelector('#link-builder-cancel');
    const createBtn = builder.querySelector('#link-builder-create');
    const modeButtons = builder.querySelectorAll('.link-builder-mode-btn');
    
    // Swap source and target
    if (swapBtn) {
        swapBtn.addEventListener('click', () => {
            const currentNodes = builder.dataset.nodes ? JSON.parse(builder.dataset.nodes) : [];
            if (currentNodes.length === 2) {
                // Reverse the array
                const reversed = [currentNodes[1], currentNodes[0]];
                updateLinkBuilderContent(builder, reversed);
                updatePreview(builder);
            }
        });
    }
    
    // Mode button selection for 3+ entities
    modeButtons.forEach(btn => {
        btn.addEventListener('click', () => {
            // Remove active class from all buttons
            modeButtons.forEach(b => b.classList.remove('active'));
            // Add active class to clicked button
            btn.classList.add('active');
            
            // Update mode description
            updateModeDescription(builder, btn.dataset.mode);
            // Update preview
            updatePreview(builder);
        });
    });
    
    // Update description and preview when type changes
    if (typeSelect) {
        typeSelect.addEventListener('change', () => {
            updateTypeDescription(builder);
            updatePreview(builder);
        });
    }
    
    // Update preview when reason changes
    if (reasonInput) {
        reasonInput.addEventListener('input', () => {
            updatePreview(builder);
        });
    }
    
    // Cancel button
    if (cancelBtn) {
        cancelBtn.addEventListener('click', () => {
            hideLinkBuilder(builder);
            if (callbacks.onCancel) {
                callbacks.onCancel();
            }
        });
    }
    
    // Create button
    if (createBtn) {
        createBtn.addEventListener('click', async () => {
            await handleCreateLink(builder, callbacks);
        });
    }
}

/**
 * Update type description based on selected type
 * @param {HTMLElement} builder - The link builder element
 */
function updateTypeDescription(builder) {
    const typeSelect = builder.querySelector('#link-builder-type-select');
    const descEl = builder.querySelector('#link-builder-type-description');
    
    if (!typeSelect || !descEl) return;
    
    const selectedType = typeSelect.value;
    const linkType = LINK_TYPES.find(t => t.value === selectedType);
    
    if (linkType) {
        descEl.textContent = linkType.description;
    }
}

/**
 * Update mode description based on selected mode
 * @param {HTMLElement} builder - The link builder element
 * @param {string} mode - Selected mode (chain, hub, or mesh)
 */
function updateModeDescription(builder, mode) {
    const descEl = builder.querySelector('#link-builder-mode-description');
    if (!descEl) return;
    
    const descriptions = {
        'chain': 'Chain: Creates sequential links A→B→C→D',
        'hub': 'Hub: Creates links from all entities to one central hub entity',
        'mesh': 'Mesh: Creates links between all pairs of entities (fully connected)'
    };
    
    descEl.textContent = descriptions[mode] || '';
}

/**
 * Update preview based on current selections
 * @param {HTMLElement} builder - The link builder element
 */
function updatePreview(builder) {
    const previewEl = builder.querySelector('#link-builder-preview-text');
    const typeSelect = builder.querySelector('#link-builder-type-select');
    const reasonInput = builder.querySelector('#link-builder-reason-input');
    
    if (!previewEl || !typeSelect) return;
    
    const nodes = builder.dataset.nodes ? JSON.parse(builder.dataset.nodes) : [];
    
    // Handle 2-entity mode
    if (nodes.length === 2) {
        updateTwoEntityPreview(builder, previewEl, typeSelect, reasonInput, nodes);
        return;
    }
    
    // Handle 3+ entity mode
    if (nodes.length >= 3) {
        updateMultiEntityPreview(builder, previewEl, typeSelect, reasonInput, nodes);
        return;
    }
    
    // No nodes or invalid state
    previewEl.innerHTML = '<em>No entities selected</em>';
}

/**
 * Update preview for 2-entity mode
 */
function updateTwoEntityPreview(builder, previewEl, typeSelect, reasonInput, nodes) {
    const selectedType = typeSelect.value;
    const linkType = LINK_TYPES.find(t => t.value === selectedType);
    const reason = reasonInput ? reasonInput.value.trim() : '';
    
    const sourceName = nodes[0].short_name || nodes[0].title || nodes[0].id;
    const targetName = nodes[1].short_name || nodes[1].title || nodes[1].id;
    
    let previewText = `<strong>${sourceName}</strong> ${linkType ? linkType.label.toLowerCase() : selectedType} <strong>${targetName}</strong>`;
    
    if (reason) {
        previewText += `<br><em>Reason: ${escapeHtml(reason)}</em>`;
    }
    
    previewEl.innerHTML = previewText;
}

/**
 * Update preview for multi-entity mode (3+)
 */
function updateMultiEntityPreview(builder, previewEl, typeSelect, reasonInput, nodes) {
    const selectedType = typeSelect.value;
    const linkType = LINK_TYPES.find(t => t.value === selectedType);
    const reason = reasonInput ? reasonInput.value.trim() : '';
    
    // Get selected mode
    const activeMode = builder.querySelector('.link-builder-mode-btn.active');
    const mode = activeMode ? activeMode.dataset.mode : 'chain';
    
    let previewText = '';
    
    // Generate preview based on mode
    if (mode === 'chain') {
        // A→B→C→D
        const links = [];
        for (let i = 0; i < nodes.length - 1; i++) {
            const srcName = nodes[i].short_name || nodes[i].title || nodes[i].id;
            const tgtName = nodes[i + 1].short_name || nodes[i + 1].title || nodes[i + 1].id;
            links.push(`<strong>${srcName}</strong> → <strong>${tgtName}</strong>`);
        }
        previewText = `<div class="preview-chain">${links.join('<br>')}</div>`;
        previewText = `<strong>${nodes.length - 1} links:</strong><br>${previewText}`;
    } else if (mode === 'hub') {
        // All→One (last entity is the hub)
        const hubNode = nodes[nodes.length - 1];
        const hubName = hubNode.short_name || hubNode.title || hubNode.id;
        const spokeNames = nodes.slice(0, -1).map(n => 
            `<strong>${n.short_name || n.title || n.id}</strong>`
        );
        previewText = `<strong>Hub:</strong> ${hubName}<br>`;
        previewText += `<strong>${nodes.length - 1} links from:</strong><br>`;
        previewText += spokeNames.join(', ');
    } else if (mode === 'mesh') {
        // All↔All (every pair connected)
        const linkCount = (nodes.length * (nodes.length - 1)) / 2;
        const nodeNames = nodes.map(n => n.short_name || n.title || n.id).join(', ');
        previewText = `<strong>${linkCount} links</strong> connecting all pairs<br>`;
        previewText += `<em>Entities: ${nodeNames}</em>`;
    }
    
    if (reason) {
        previewText += `<br><br><em>Reason: ${escapeHtml(reason)}</em>`;
    }
    
    previewText += `<br><em>Link type: ${linkType ? linkType.label : selectedType}</em>`;
    
    previewEl.innerHTML = previewText;
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

/**
 * Handle creating the link
 * @param {HTMLElement} builder - The link builder element
 * @param {Object} callbacks - Callback functions
 */
async function handleCreateLink(builder, callbacks) {
    const typeSelect = builder.querySelector('#link-builder-type-select');
    const reasonInput = builder.querySelector('#link-builder-reason-input');
    const createBtn = builder.querySelector('#link-builder-create');
    
    const nodes = builder.dataset.nodes ? JSON.parse(builder.dataset.nodes) : [];
    if (nodes.length < 2) {
        console.error('Link builder requires at least 2 nodes');
        return;
    }
    
    // Disable create button during request
    if (createBtn) {
        createBtn.disabled = true;
        createBtn.textContent = 'Creating...';
    }
    
    try {
        // Handle 2-entity mode (single link)
        if (nodes.length === 2) {
            const linkData = {
                source: nodes[0].id,
                target: nodes[1].id,
                edge_type: typeSelect.value,
                reason: reasonInput.value.trim() || undefined
            };
            
            if (callbacks.onCreate) {
                await callbacks.onCreate(linkData);
            }
        } 
        // Handle 3+ entity mode (batch links)
        else {
            const links = generateLinksFromMode(builder, nodes, typeSelect.value, reasonInput.value.trim());
            
            if (callbacks.onCreate) {
                // Call onCreate with batch link data
                await callbacks.onCreate({
                    batch: true,
                    links: links
                });
            }
        }
        
        // Success - hide the builder
        hideLinkBuilder(builder);
    } catch (error) {
        console.error('Failed to create link:', error);
        alert('Failed to create link: ' + error.message);
    } finally {
        // Re-enable create button
        if (createBtn) {
            createBtn.disabled = false;
            createBtn.textContent = 'Create Link';
        }
    }
}

/**
 * Generate links based on selected mode
 * @param {HTMLElement} builder - The link builder element
 * @param {Array} nodes - Array of nodes
 * @param {string} edgeType - Selected edge type
 * @param {string} reason - Optional reason
 * @returns {Array} Array of link objects
 */
function generateLinksFromMode(builder, nodes, edgeType, reason) {
    const activeMode = builder.querySelector('.link-builder-mode-btn.active');
    const mode = activeMode ? activeMode.dataset.mode : 'chain';
    
    const links = [];
    
    if (mode === 'chain') {
        // Sequential: A→B→C→D
        for (let i = 0; i < nodes.length - 1; i++) {
            links.push({
                source: nodes[i].id,
                target: nodes[i + 1].id,
                edge_type: edgeType,
                reason: reason || undefined
            });
        }
    } else if (mode === 'hub') {
        // Hub: All point to last entity
        const hubNode = nodes[nodes.length - 1];
        for (let i = 0; i < nodes.length - 1; i++) {
            links.push({
                source: nodes[i].id,
                target: hubNode.id,
                edge_type: edgeType,
                reason: reason || undefined
            });
        }
    } else if (mode === 'mesh') {
        // Mesh: All pairs connected
        for (let i = 0; i < nodes.length; i++) {
            for (let j = i + 1; j < nodes.length; j++) {
                links.push({
                    source: nodes[i].id,
                    target: nodes[j].id,
                    edge_type: edgeType,
                    reason: reason || undefined
                });
            }
        }
    }
    
    return links;
}

/**
 * Show the link builder
 * @param {HTMLElement} builder - The link builder element
 */
export function showLinkBuilder(builder) {
    if (builder) {
        builder.classList.add('visible');
    }
}

/**
 * Hide the link builder
 * @param {HTMLElement} builder - The link builder element
 */
export function hideLinkBuilder(builder) {
    if (builder) {
        builder.classList.remove('visible');
        
        // Clear reason input
        const reasonInput = builder.querySelector('#link-builder-reason-input');
        if (reasonInput) {
            reasonInput.value = '';
        }
    }
}

/**
 * Update link builder content with selected nodes
 * @param {HTMLElement} builder - The link builder element
 * @param {Array} nodes - Array of 2+ node objects
 */
export function updateLinkBuilderContent(builder, nodes) {
    if (!builder || !nodes || nodes.length < 2) {
        hideLinkBuilder(builder);
        return;
    }
    
    // Store nodes data
    builder.dataset.nodes = JSON.stringify(nodes);
    
    // Get UI elements
    const twoEntitySection = builder.querySelector('.link-builder-nodes');
    const multiEntitySection = builder.querySelector('.link-builder-multi-nodes');
    const modeSection = builder.querySelector('.link-builder-mode');
    
    // Handle 2-entity mode
    if (nodes.length === 2) {
        // Show two-entity UI
        if (twoEntitySection) twoEntitySection.style.display = 'flex';
        if (multiEntitySection) multiEntitySection.style.display = 'none';
        if (modeSection) modeSection.style.display = 'none';
        
        // Update source node info
        const sourceInfo = builder.querySelector('#link-builder-source .link-builder-node-info');
        if (sourceInfo) {
            sourceInfo.innerHTML = '';
            const sourceId = createClickableId(nodes[0].id);
            const sourceTitle = document.createElement('span');
            sourceTitle.className = 'link-builder-node-title';
            sourceTitle.textContent = nodes[0].short_name || nodes[0].title || 'Untitled';
            
            sourceInfo.appendChild(sourceId);
            sourceInfo.appendChild(sourceTitle);
        }
        
        // Update target node info
        const targetInfo = builder.querySelector('#link-builder-target .link-builder-node-info');
        if (targetInfo) {
            targetInfo.innerHTML = '';
            const targetId = createClickableId(nodes[1].id);
            const targetTitle = document.createElement('span');
            targetTitle.className = 'link-builder-node-title';
            targetTitle.textContent = nodes[1].short_name || nodes[1].title || 'Untitled';
            
            targetInfo.appendChild(targetId);
            targetInfo.appendChild(targetTitle);
        }
    } 
    // Handle 3+ entity mode
    else {
        // Show multi-entity UI
        if (twoEntitySection) twoEntitySection.style.display = 'none';
        if (multiEntitySection) multiEntitySection.style.display = 'block';
        if (modeSection) modeSection.style.display = 'block';
        
        // Update node count
        const nodeCountEl = builder.querySelector('#link-builder-node-count');
        if (nodeCountEl) {
            nodeCountEl.textContent = nodes.length;
        }
        
        // Update node list
        const nodeListEl = builder.querySelector('#link-builder-node-list');
        if (nodeListEl) {
            nodeListEl.innerHTML = '';
            nodes.forEach(node => {
                const nodeItem = document.createElement('div');
                nodeItem.className = 'link-builder-node-item';
                
                const nodeId = createClickableId(node.id);
                const nodeTitle = document.createElement('span');
                nodeTitle.className = 'link-builder-node-title';
                nodeTitle.textContent = node.short_name || node.title || 'Untitled';
                
                nodeItem.appendChild(nodeId);
                nodeItem.appendChild(nodeTitle);
                nodeListEl.appendChild(nodeItem);
            });
        }
        
        // Reset mode to chain
        const modeButtons = builder.querySelectorAll('.link-builder-mode-btn');
        modeButtons.forEach(btn => {
            if (btn.dataset.mode === 'chain') {
                btn.classList.add('active');
            } else {
                btn.classList.remove('active');
            }
        });
        
        // Update mode description
        updateModeDescription(builder, 'chain');
    }
    
    // Reset to default type
    const typeSelect = builder.querySelector('#link-builder-type-select');
    if (typeSelect) {
        typeSelect.value = 'depends_on';
    }
    
    // Update description and preview
    updateTypeDescription(builder);
    updatePreview(builder);
    
    // Show the builder
    showLinkBuilder(builder);
}
