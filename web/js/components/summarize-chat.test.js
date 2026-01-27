/**
 * Summarize Chat Component Tests
 */

import { describe, it, expect, beforeEach, afterEach } from '../test-utils.js';
import { 
    createSummarizeChatModal,
    showSummarizeChatModal,
    hideSummarizeChatModal,
    addChatMessage,
    initSummarizeChatModal
} from './summarize-chat.js';

describe('SummarizeChatModal', () => {
    let container;
    let modal;
    
    beforeEach(() => {
        // Create container
        container = document.createElement('div');
        container.id = 'test-container';
        document.body.appendChild(container);
        
        // Create and mount modal
        modal = createSummarizeChatModal();
        container.appendChild(modal);
        initSummarizeChatModal();
        
        // Mock getNode function
        window.getNode = (id) => ({
            id,
            type: 'task',
            title: `Test Task ${id}`,
            short_name: `Task ${id}`,
            status: 'pending',
            priority: 2
        });
    });
    
    afterEach(() => {
        // Clean up
        if (container && container.parentNode) {
            container.parentNode.removeChild(container);
        }
        delete window.getNode;
    });
    
    describe('createSummarizeChatModal', () => {
        it('should create modal with correct structure', () => {
            expect(modal).toBeDefined();
            expect(modal.className).toBe('summarize-chat-overlay hidden');
            expect(modal.id).toBe('summarize-chat-modal');
        });
        
        it('should have all required elements', () => {
            const modalContent = modal.querySelector('.summarize-chat-modal');
            expect(modalContent).toBeDefined();
            
            const header = modal.querySelector('.summarize-chat-header');
            expect(header).toBeDefined();
            
            const messages = modal.querySelector('.summarize-chat-messages');
            expect(messages).toBeDefined();
            
            const input = modal.querySelector('.summarize-chat-input');
            expect(input).toBeDefined();
            
            const sendBtn = modal.querySelector('.summarize-chat-send');
            expect(sendBtn).toBeDefined();
        });
    });
    
    describe('showSummarizeChatModal', () => {
        it('should show modal and update title', () => {
            showSummarizeChatModal(['bn-0001', 'bn-0002'], false);
            
            expect(modal.classList.contains('hidden')).toBe(false);
            
            const title = modal.querySelector('#summarize-chat-title');
            expect(title.textContent).toBe('Summarizing 2 entities');
        });
        
        it('should add initial messages', () => {
            showSummarizeChatModal(['bn-0001'], false);
            
            const messages = modal.querySelector('#summarize-chat-messages');
            const chatMessages = messages.querySelectorAll('.chat-message');
            
            expect(chatMessages.length).toBeGreaterThan(0);
        });
        
        it('should disable input in readonly mode', () => {
            showSummarizeChatModal(['bn-0001'], true);
            
            const input = modal.querySelector('#summarize-chat-input');
            const sendBtn = modal.querySelector('#summarize-chat-send');
            
            expect(input.disabled).toBe(true);
            expect(sendBtn.disabled).toBe(true);
        });
        
        it('should enable input in normal mode', () => {
            showSummarizeChatModal(['bn-0001'], false);
            
            const input = modal.querySelector('#summarize-chat-input');
            const sendBtn = modal.querySelector('#summarize-chat-send');
            
            expect(input.disabled).toBe(false);
            expect(sendBtn.disabled).toBe(false);
        });
    });
    
    describe('hideSummarizeChatModal', () => {
        it('should hide modal', () => {
            showSummarizeChatModal(['bn-0001'], false);
            expect(modal.classList.contains('hidden')).toBe(false);
            
            hideSummarizeChatModal();
            expect(modal.classList.contains('hidden')).toBe(true);
        });
    });
    
    describe('addChatMessage', () => {
        it('should add user message', () => {
            const messagesEl = modal.querySelector('#summarize-chat-messages');
            messagesEl.innerHTML = '';
            
            addChatMessage(messagesEl, 'Test message', 'user');
            
            const messages = messagesEl.querySelectorAll('.chat-message');
            expect(messages.length).toBe(1);
            
            const message = messages[0];
            expect(message.classList.contains('chat-message-user')).toBe(true);
            expect(message.textContent).toContain('Test message');
        });
        
        it('should add agent message', () => {
            const messagesEl = modal.querySelector('#summarize-chat-messages');
            messagesEl.innerHTML = '';
            
            addChatMessage(messagesEl, '<p>Agent response</p>', 'agent');
            
            const messages = messagesEl.querySelectorAll('.chat-message');
            expect(messages.length).toBe(1);
            
            const message = messages[0];
            expect(message.classList.contains('chat-message-agent')).toBe(true);
        });
        
        it('should show quick actions when suggestions provided', () => {
            const messagesEl = modal.querySelector('#summarize-chat-messages');
            const suggestions = [
                { label: 'Action 1', action: 'action-1', data: {} },
                { label: 'Action 2', action: 'action-2', data: {} }
            ];
            
            addChatMessage(messagesEl, 'Message with actions', 'agent', suggestions);
            
            const quickActions = modal.querySelector('#summarize-chat-quick-actions');
            expect(quickActions.style.display).not.toBe('none');
            
            const buttons = quickActions.querySelectorAll('.quick-action-btn');
            expect(buttons.length).toBe(2);
        });
    });
    
    describe('Event Handling', () => {
        it('should close on close button click', () => {
            showSummarizeChatModal(['bn-0001'], false);
            
            const closeBtn = modal.querySelector('#summarize-chat-close');
            closeBtn.click();
            
            expect(modal.classList.contains('hidden')).toBe(true);
        });
        
        it('should close on overlay click', () => {
            showSummarizeChatModal(['bn-0001'], false);
            
            modal.click();
            
            expect(modal.classList.contains('hidden')).toBe(true);
        });
        
        it('should not close on modal content click', () => {
            showSummarizeChatModal(['bn-0001'], false);
            
            const modalContent = modal.querySelector('.summarize-chat-modal');
            modalContent.click();
            
            // Modal should still be visible
            expect(modal.classList.contains('hidden')).toBe(false);
        });
        
        it('should dispatch chat-message event on send', (done) => {
            const input = modal.querySelector('#summarize-chat-input');
            const sendBtn = modal.querySelector('#summarize-chat-send');
            
            modal.addEventListener('chat-message', (e) => {
                expect(e.detail.message).toBe('Test question');
                done();
            });
            
            input.value = 'Test question';
            sendBtn.click();
        });
        
        it('should dispatch chat-action event on quick action click', (done) => {
            const messagesEl = modal.querySelector('#summarize-chat-messages');
            const suggestions = [
                { label: 'Test Action', action: 'test-action', data: { foo: 'bar' } }
            ];
            
            addChatMessage(messagesEl, 'Message', 'agent', suggestions);
            
            modal.addEventListener('chat-action', (e) => {
                expect(e.detail.action).toBe('test-action');
                expect(e.detail.data).toEqual({ foo: 'bar' });
                done();
            });
            
            const quickActionBtn = modal.querySelector('.quick-action-btn');
            quickActionBtn.click();
        });
    });
});
