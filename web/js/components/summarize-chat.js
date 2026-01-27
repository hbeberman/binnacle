/**
 * Summarize Agent Chat Component
 * 
 * Interactive chat modal for AI agent summarization of selected entities.
 * The agent receives full context of selected entities and can:
 * - Provide summaries and analysis
 * - Answer questions about the selection
 * - Suggest actions and relationships
 * - Help with prioritization
 */

import { getNode } from '../state.js';
import { createClickableId } from '../utils/clickable-ids.js';

/**
 * Create the summarize chat modal HTML
 * @returns {HTMLElement} The modal overlay element
 */
export function createSummarizeChatModal() {
    const overlay = document.createElement('div');
    overlay.className = 'summarize-chat-overlay hidden';
    overlay.id = 'summarize-chat-modal';
    
    overlay.innerHTML = `
        <div class="summarize-chat-modal">
            <div class="summarize-chat-header">
                <div class="summarize-chat-title-section">
                    <span class="summarize-chat-icon">📊</span>
                    <h2 class="summarize-chat-title" id="summarize-chat-title">Summarizing Entities</h2>
                </div>
                <button class="summarize-chat-close" id="summarize-chat-close" title="Close">&times;</button>
            </div>
            <div class="summarize-chat-messages" id="summarize-chat-messages">
                <div class="summarize-chat-loading">Initializing agent...</div>
            </div>
            <div class="summarize-chat-quick-actions" id="summarize-chat-quick-actions" style="display: none;">
                <!-- Quick action buttons populated dynamically -->
            </div>
            <div class="summarize-chat-input-container">
                <input 
                    type="text" 
                    class="summarize-chat-input" 
                    id="summarize-chat-input" 
                    placeholder="Ask a question about the selection..."
                    autocomplete="off"
                />
                <button class="summarize-chat-send" id="summarize-chat-send" title="Send">
                    Send
                </button>
            </div>
        </div>
    `;
    
    return overlay;
}

/**
 * Escape HTML to prevent XSS
 * @param {string} str - String to escape
 * @returns {string} Escaped string
 */
function escapeHtml(str) {
    if (!str) return '';
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

/**
 * Format a timestamp for display
 * @param {Date} date - Date object
 * @returns {string} Formatted time
 */
function formatTime(date) {
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

/**
 * Add a message to the chat
 * @param {HTMLElement} messagesContainer - Messages container element
 * @param {string} content - Message content (HTML allowed for agent messages)
 * @param {string} role - 'user' or 'agent'
 * @param {Array} suggestions - Optional array of suggested actions
 */
export function addChatMessage(messagesContainer, content, role = 'agent', suggestions = []) {
    const messageDiv = document.createElement('div');
    messageDiv.className = `chat-message chat-message-${role}`;
    
    const timestamp = formatTime(new Date());
    const icon = role === 'agent' ? '🤖' : '👤';
    const label = role === 'agent' ? 'Agent' : 'You';
    
    messageDiv.innerHTML = `
        <div class="chat-message-header">
            <span class="chat-message-icon">${icon}</span>
            <span class="chat-message-label">${label}</span>
            <span class="chat-message-time">${timestamp}</span>
        </div>
        <div class="chat-message-content">${role === 'agent' ? content : escapeHtml(content)}</div>
    `;
    
    messagesContainer.appendChild(messageDiv);
    
    // If there are suggestions, update the quick actions section
    if (suggestions && suggestions.length > 0) {
        updateQuickActions(suggestions);
    }
    
    // Scroll to bottom
    messagesContainer.scrollTop = messagesContainer.scrollHeight;
}

/**
 * Update quick action buttons
 * @param {Array} suggestions - Array of suggestion objects {label, action, data}
 */
function updateQuickActions(suggestions) {
    const quickActionsEl = document.getElementById('summarize-chat-quick-actions');
    if (!quickActionsEl) return;
    
    if (!suggestions || suggestions.length === 0) {
        quickActionsEl.style.display = 'none';
        return;
    }
    
    quickActionsEl.innerHTML = '<div class="quick-actions-label">Suggested actions:</div>';
    
    const actionsContainer = document.createElement('div');
    actionsContainer.className = 'quick-actions-buttons';
    
    suggestions.forEach(suggestion => {
        const btn = document.createElement('button');
        btn.className = 'quick-action-btn';
        btn.textContent = suggestion.label;
        btn.dataset.action = suggestion.action;
        btn.dataset.data = JSON.stringify(suggestion.data || {});
        
        btn.addEventListener('click', () => {
            handleQuickAction(suggestion);
        });
        
        actionsContainer.appendChild(btn);
    });
    
    quickActionsEl.appendChild(actionsContainer);
    quickActionsEl.style.display = 'block';
}

/**
 * Handle quick action button click
 * @param {Object} suggestion - Suggestion object
 */
async function handleQuickAction(suggestion) {
    const modal = document.getElementById('summarize-chat-modal');
    if (!modal) return;
    
    const messagesEl = document.getElementById('summarize-chat-messages');
    
    // Show processing feedback
    const processingMsg = document.createElement('div');
    processingMsg.className = 'chat-message chat-message-agent processing';
    processingMsg.innerHTML = `
        <div class="chat-message-header">
            <span class="chat-message-icon">⚙️</span>
            <span class="chat-message-label">Processing</span>
        </div>
        <div class="chat-message-content">Executing action: ${escapeHtml(suggestion.label)}...</div>
    `;
    messagesEl.appendChild(processingMsg);
    messagesEl.scrollTop = messagesEl.scrollHeight;
    
    try {
        // Call the API to execute the action
        const response = await fetch('/api/summarize/action', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                action_type: suggestion.action,
                params: suggestion.data || {}
            })
        });
        
        // Remove processing message
        processingMsg.remove();
        
        if (!response.ok) {
            const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
            throw new Error(errorData.error || `HTTP ${response.status}`);
        }
        
        const result = await response.json();
        
        // Show success message
        const successHtml = `<p>✓ ${escapeHtml(suggestion.label)} completed successfully.</p>`;
        addChatMessage(messagesEl, successHtml, 'agent');
        
        // Dispatch custom event for parent to handle UI updates
        modal.dispatchEvent(new CustomEvent('chat-action', {
            detail: {
                action: suggestion.action,
                data: suggestion.data,
                result: result
            }
        }));
        
    } catch (error) {
        // Remove processing message
        processingMsg.remove();
        
        // Show error message
        const errorHtml = `<p>❌ Failed to execute action: ${escapeHtml(error.message)}</p>`;
        addChatMessage(messagesEl, errorHtml, 'agent');
        
        console.error('Action execution failed:', error);
    }
}

/**
 * Build context summary from selected nodes
 * @param {Array} selectedNodes - Array of node objects
 * @returns {string} HTML context summary
 */
function buildContextSummary(selectedNodes) {
    const typeCounts = {};
    selectedNodes.forEach(node => {
        const type = node.type || 'unknown';
        typeCounts[type] = (typeCounts[type] || 0) + 1;
    });
    
    const summaryParts = Object.entries(typeCounts)
        .map(([type, count]) => {
            const plural = count > 1 ? 's' : '';
            return `${count} ${type}${plural}`;
        })
        .sort()
        .join(', ');
    
    let html = `<p>You've selected ${summaryParts}:</p>`;
    
    // Group by type
    const byType = {};
    selectedNodes.forEach(node => {
        const type = node.type || 'unknown';
        if (!byType[type]) byType[type] = [];
        byType[type].push(node);
    });
    
    // Render each type section
    Object.entries(byType).forEach(([type, nodes]) => {
        const typeLabel = type.charAt(0).toUpperCase() + type.slice(1);
        const plural = nodes.length > 1 ? 's' : '';
        
        html += `<p><strong>${typeLabel}${plural}:</strong></p><ul>`;
        
        nodes.forEach(node => {
            const title = node.short_name || node.title || node.name || 'Untitled';
            const status = node.status ? ` (${node.status})` : '';
            const priority = node.priority !== undefined ? `, P${node.priority}` : '';
            const queued = node.queued ? ', ⏰ queued' : '';
            
            // Create a temporary container to render the clickable ID
            const tempDiv = document.createElement('div');
            const clickableId = createClickableId(node.id);
            tempDiv.appendChild(clickableId);
            
            html += `<li>${tempDiv.innerHTML}: ${escapeHtml(title)}${escapeHtml(status + priority + queued)}</li>`;
        });
        
        html += `</ul>`;
    });
    
    return html;
}

/**
 * Show the summarize chat modal with selected entities
 * @param {Array} selectedNodeIds - Array of node IDs
 * @param {boolean} readonly - Whether in readonly mode
 */
export async function showSummarizeChatModal(selectedNodeIds, readonly = false) {
    const overlay = document.getElementById('summarize-chat-modal');
    if (!overlay) {
        console.error('Summarize chat modal overlay not found in DOM');
        return;
    }
    
    // Get full node objects
    const selectedNodes = selectedNodeIds.map(id => getNode(id)).filter(Boolean);
    
    if (selectedNodes.length === 0) {
        console.error('No valid nodes selected');
        return;
    }
    
    // Update title
    const titleEl = document.getElementById('summarize-chat-title');
    titleEl.textContent = `Summarizing ${selectedNodes.length} ${selectedNodes.length === 1 ? 'entity' : 'entities'}`;
    
    // Clear messages
    const messagesEl = document.getElementById('summarize-chat-messages');
    messagesEl.innerHTML = '<div class="summarize-chat-loading">Initializing agent...</div>';
    
    // Show overlay
    overlay.classList.remove('hidden');
    
    try {
        // Start a summarize session with the backend
        const response = await fetch('/api/summarize/start', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                entity_ids: selectedNodeIds
            })
        });
        
        if (!response.ok) {
            const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
            throw new Error(errorData.error || `HTTP ${response.status}`);
        }
        
        // Clear loading message
        messagesEl.innerHTML = '';
        
        // Add initial agent message with context
        const contextSummary = buildContextSummary(selectedNodes);
        addChatMessage(messagesEl, contextSummary, 'agent');
        
        // Add initial analysis with quick actions
        const analysisHtml = `<p>I can help you with:</p>
<ul>
    <li>Understanding the current state and relationships</li>
    <li>Identifying blockers and dependencies</li>
    <li>Suggesting next actions or priorities</li>
    <li>Answering questions about this selection</li>
</ul>
<p>What would you like to know?</p>`;
        
        addChatMessage(messagesEl, analysisHtml, 'agent', [
            { label: 'Show critical path', action: 'show-critical-path', data: { nodeIds: selectedNodeIds } },
            { label: 'Find blockers', action: 'find-blockers', data: { nodeIds: selectedNodeIds } },
            { label: 'Export summary', action: 'export-summary', data: { nodeIds: selectedNodeIds } }
        ]);
        
    } catch (error) {
        // Show error message
        messagesEl.innerHTML = `<div class="summarize-chat-error">Failed to start session: ${escapeHtml(error.message)}</div>`;
        console.error('Failed to start summarize session:', error);
        
        // Close modal after a delay
        setTimeout(() => {
            hideSummarizeChatModal();
        }, 3000);
        return;
    }
    
    // Handle readonly mode
    const inputEl = document.getElementById('summarize-chat-input');
    const sendBtn = document.getElementById('summarize-chat-send');
    
    if (readonly) {
        inputEl.disabled = true;
        inputEl.placeholder = 'Chat is readonly';
        sendBtn.disabled = true;
    } else {
        inputEl.disabled = false;
        inputEl.placeholder = 'Ask a question about the selection...';
        sendBtn.disabled = false;
        inputEl.focus();
    }
}

/**
 * Hide the summarize chat modal
 */
export function hideSummarizeChatModal() {
    const overlay = document.getElementById('summarize-chat-modal');
    if (overlay) {
        overlay.classList.add('hidden');
        
        // Clear quick actions
        const quickActionsEl = document.getElementById('summarize-chat-quick-actions');
        if (quickActionsEl) {
            quickActionsEl.style.display = 'none';
            quickActionsEl.innerHTML = '';
        }
    }
}

/**
 * Handle sending a message
 * @param {string} message - User message
 */
async function handleSendMessage(message) {
    if (!message || message.trim() === '') return;
    
    const messagesEl = document.getElementById('summarize-chat-messages');
    if (!messagesEl) return;
    
    // Add user message
    addChatMessage(messagesEl, message, 'user');
    
    // Clear input
    const inputEl = document.getElementById('summarize-chat-input');
    if (inputEl) {
        inputEl.value = '';
    }
    
    // Show typing indicator
    const typingIndicator = document.createElement('div');
    typingIndicator.className = 'chat-message chat-message-agent typing';
    typingIndicator.innerHTML = `
        <div class="chat-message-header">
            <span class="chat-message-icon">🤖</span>
            <span class="chat-message-label">Agent</span>
        </div>
        <div class="chat-message-content">Thinking...</div>
    `;
    messagesEl.appendChild(typingIndicator);
    messagesEl.scrollTop = messagesEl.scrollHeight;
    
    try {
        // Call the chat API
        const response = await fetch('/api/summarize/chat', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                message: message
            })
        });
        
        // Remove typing indicator
        typingIndicator.remove();
        
        if (!response.ok) {
            const errorData = await response.json().catch(() => ({ error: 'Unknown error' }));
            throw new Error(errorData.error || `HTTP ${response.status}`);
        }
        
        const result = await response.json();
        
        // Add agent response with suggested actions
        const agentResponse = result.response || 'No response from agent';
        const suggestions = result.suggested_actions || [];
        
        addChatMessage(messagesEl, agentResponse, 'agent', suggestions);
        
        // Dispatch event for parent to handle
        const modal = document.getElementById('summarize-chat-modal');
        if (modal) {
            modal.dispatchEvent(new CustomEvent('chat-message', {
                detail: { 
                    message,
                    response: agentResponse,
                    suggestions
                }
            }));
        }
        
    } catch (error) {
        // Remove typing indicator
        typingIndicator.remove();
        
        // Show error message
        const errorHtml = `<p>❌ Failed to get response: ${escapeHtml(error.message)}</p>`;
        addChatMessage(messagesEl, errorHtml, 'agent');
        
        console.error('Chat message failed:', error);
    }
}

/**
 * Initialize the summarize chat modal with event handlers
 */
export function initSummarizeChatModal() {
    const overlay = document.getElementById('summarize-chat-modal');
    if (!overlay) {
        console.error('Summarize chat modal overlay not found in DOM');
        return;
    }
    
    // Close button
    const closeBtn = document.getElementById('summarize-chat-close');
    closeBtn.addEventListener('click', hideSummarizeChatModal);
    
    // Close on overlay click (but not on content click)
    overlay.addEventListener('click', (e) => {
        if (e.target === overlay) {
            hideSummarizeChatModal();
        }
    });
    
    // Close on Escape key
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && !overlay.classList.contains('hidden')) {
            hideSummarizeChatModal();
        }
    });
    
    // Send button
    const sendBtn = document.getElementById('summarize-chat-send');
    sendBtn.addEventListener('click', () => {
        const inputEl = document.getElementById('summarize-chat-input');
        if (inputEl) {
            handleSendMessage(inputEl.value);
        }
    });
    
    // Enter key to send
    const inputEl = document.getElementById('summarize-chat-input');
    inputEl.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleSendMessage(inputEl.value);
        }
    });
}

/**
 * Mount the summarize chat modal to the DOM
 * @param {HTMLElement|string} target - Target element or selector
 */
export function mountSummarizeChatModal(target) {
    const container = typeof target === 'string' 
        ? document.querySelector(target) 
        : target;
    
    if (!container) {
        console.error('Summarize chat modal target not found');
        return;
    }
    
    const modal = createSummarizeChatModal();
    container.appendChild(modal);
    initSummarizeChatModal();
}
