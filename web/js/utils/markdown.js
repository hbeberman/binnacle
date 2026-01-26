/**
 * Simple Markdown Parser
 * 
 * Supports:
 * - Headings (# to ######)
 * - Lists (unordered -, *, + and ordered 1.)
 * - Code blocks (```language ... ```)
 * - Inline code (`code`)
 * - Links ([text](url))
 * - Bold (**text** or __text__)
 * - Italic (*text* or _text_)
 * - Paragraphs
 */

/**
 * Escape HTML special characters
 * @param {string} text - Text to escape
 * @returns {string} Escaped text
 */
function escapeHtml(text) {
    const replacements = {
        '&': '&amp;',
        '<': '&lt;',
        '>': '&gt;',
        '"': '&quot;',
        "'": '&#39;'
    };
    return text.replace(/[&<>"']/g, char => replacements[char]);
}

/**
 * Process inline markdown (bold, italic, code, links)
 * @param {string} text - Text to process
 * @returns {string} HTML with inline elements
 */
function processInline(text) {
    let result = escapeHtml(text);
    
    // Inline code (must be before bold/italic to avoid conflicts)
    result = result.replace(/`([^`]+)`/g, '<code>$1</code>');
    
    // Links [text](url)
    result = result.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" rel="noopener">$1</a>');
    
    // Bold **text** or __text__
    result = result.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
    result = result.replace(/__([^_]+)__/g, '<strong>$1</strong>');
    
    // Italic *text* or _text_ (must be after bold)
    result = result.replace(/\*([^*]+)\*/g, '<em>$1</em>');
    result = result.replace(/_([^_]+)_/g, '<em>$1</em>');
    
    return result;
}

/**
 * Parse markdown to HTML
 * @param {string} markdown - Markdown text
 * @returns {string} Rendered HTML
 */
export function parseMarkdown(markdown) {
    if (!markdown) return '';
    
    const lines = markdown.split('\n');
    const html = [];
    let inCodeBlock = false;
    let codeBlockLines = [];
    let codeBlockLanguage = '';
    let inList = false;
    let listType = null; // 'ul' or 'ol'
    let listItems = [];
    
    for (let i = 0; i < lines.length; i++) {
        const line = lines[i];
        const trimmed = line.trim();
        
        // Code blocks
        if (trimmed.startsWith('```')) {
            if (inCodeBlock) {
                // End code block
                const code = escapeHtml(codeBlockLines.join('\n'));
                const langClass = codeBlockLanguage ? ` class="language-${codeBlockLanguage}"` : '';
                html.push(`<pre><code${langClass}>${code}</code></pre>`);
                inCodeBlock = false;
                codeBlockLines = [];
                codeBlockLanguage = '';
            } else {
                // Start code block
                // Close any open list
                if (inList) {
                    html.push(`</${listType}>`);
                    inList = false;
                    listItems = [];
                }
                inCodeBlock = true;
                codeBlockLanguage = trimmed.slice(3).trim();
            }
            continue;
        }
        
        if (inCodeBlock) {
            codeBlockLines.push(line);
            continue;
        }
        
        // Headings
        const headingMatch = trimmed.match(/^(#{1,6})\s+(.+)$/);
        if (headingMatch) {
            // Close any open list
            if (inList) {
                html.push(`</${listType}>`);
                inList = false;
                listItems = [];
            }
            const level = headingMatch[1].length;
            const text = processInline(headingMatch[2]);
            html.push(`<h${level}>${text}</h${level}>`);
            continue;
        }
        
        // Unordered list items
        const ulMatch = trimmed.match(/^[-*+]\s+(.+)$/);
        if (ulMatch) {
            if (!inList || listType !== 'ul') {
                if (inList) {
                    html.push(`</${listType}>`);
                }
                html.push('<ul>');
                inList = true;
                listType = 'ul';
            }
            const text = processInline(ulMatch[1]);
            html.push(`<li>${text}</li>`);
            continue;
        }
        
        // Ordered list items
        const olMatch = trimmed.match(/^\d+\.\s+(.+)$/);
        if (olMatch) {
            if (!inList || listType !== 'ol') {
                if (inList) {
                    html.push(`</${listType}>`);
                }
                html.push('<ol>');
                inList = true;
                listType = 'ol';
            }
            const text = processInline(olMatch[1]);
            html.push(`<li>${text}</li>`);
            continue;
        }
        
        // Empty line
        if (trimmed === '') {
            if (inList) {
                html.push(`</${listType}>`);
                inList = false;
                listItems = [];
            }
            html.push('');
            continue;
        }
        
        // Regular paragraph
        if (inList) {
            html.push(`</${listType}>`);
            inList = false;
            listItems = [];
        }
        const text = processInline(trimmed);
        html.push(`<p>${text}</p>`);
    }
    
    // Close any remaining open list
    if (inList) {
        html.push(`</${listType}>`);
    }
    
    // Close any remaining open code block
    if (inCodeBlock) {
        const code = escapeHtml(codeBlockLines.join('\n'));
        const langClass = codeBlockLanguage ? ` class="language-${codeBlockLanguage}"` : '';
        html.push(`<pre><code${langClass}>${code}</code></pre>`);
    }
    
    return html.join('\n');
}

/**
 * Render markdown content to a DOM element
 * @param {HTMLElement} element - Target element
 * @param {string} markdown - Markdown content
 */
export function renderMarkdown(element, markdown) {
    if (!element) {
        console.error('Target element is required for markdown rendering');
        return;
    }
    
    element.innerHTML = parseMarkdown(markdown);
}
