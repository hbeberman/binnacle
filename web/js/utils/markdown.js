/**
 * Markdown Parser using marked library
 * 
 * Supports:
 * - All standard Markdown features
 * - GFM (GitHub Flavored Markdown): tables, task lists, strikethrough
 * - Code syntax highlighting via highlight.js
 */

import { marked } from 'marked';
import hljs from 'highlight.js';

// Configure marked with GFM and custom renderer
const renderer = new marked.Renderer();

// Override link rendering to add target="_blank" and rel="noopener" for security
renderer.link = function(href, title, text) {
    // Build the link with attributes in the expected order
    const titleAttr = title ? ` title="${title}"` : '';
    return `<a href="${href}"${titleAttr} target="_blank" rel="noopener">${text}</a>`;
};

// Override code block rendering to add syntax highlighting
renderer.code = function(code, language) {
    if (language && hljs.getLanguage(language)) {
        try {
            const highlighted = hljs.highlight(code, { language: language }).value;
            return `<pre><code class="language-${language}">${highlighted}</code></pre>`;
        } catch (err) {
            console.error('Syntax highlighting error:', err);
        }
    }
    // No highlighting if language not recognized
    const escaped = code.replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
    return `<pre><code${language ? ` class="language-${language}"` : ''}>${escaped}</code></pre>`;
};

// Configure marked with all options in one place
marked.use({
    gfm: true,              // Enable GitHub Flavored Markdown
    breaks: false,          // Don't convert \n to <br> (GFM default)
    pedantic: false,        // Relaxed parsing
    renderer: renderer,
    // Hooks to post-process the output
    hooks: {
        postprocess(html) {
            // Remove trailing newlines in code blocks but preserve paragraph spacing
            return html
                .replace(/(<code[^>]*>)([\s\S]*?)\n(<\/code>)/g, '$1$2$3')  // Remove trailing \n before </code>
                .replace(/(<\/p>)\n(<p>)/g, '$1\n\n$2')                      // Double newline between paragraphs
                .replace(/(<\/[uo]l>)\n(<p>)/g, '$1\n\n$2')                 // Double newline between list and paragraph
                .replace(/(<\/h[1-6]>)\n(<p>)/g, '$1\n\n$2')                // Double newline between heading and paragraph
                .replace(/(<\/p>)\n(<h[1-6]>)/g, '$1\n\n$2')                // Double newline between paragraph and heading
                .replace(/(<\/h[1-6]>)\n(<h[1-6]>)/g, '$1\n\n$2')           // Double newline between headings
                .trim();                                                      // Remove leading/trailing whitespace
        }
    },
    // Sanitize HTML in walkTokens
    walkTokens(token) {
        // Sanitize any HTML in tokens
        if (token.type === 'html') {
            // Escape HTML tags to prevent XSS
            token.text = token.text
                .replace(/&/g, '&amp;')
                .replace(/</g, '&lt;')
                .replace(/>/g, '&gt;')
                .replace(/"/g, '&quot;')
                .replace(/'/g, '&#39;');
        }
    }
});

/**
 * Parse markdown to HTML
 * @param {string} markdown - Markdown text
 * @returns {string} Rendered HTML
 */
export function parseMarkdown(markdown) {
    if (!markdown) return '';
    
    try {
        return marked.parse(markdown);
    } catch (err) {
        console.error('Markdown parsing error:', err);
        return '';
    }
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
