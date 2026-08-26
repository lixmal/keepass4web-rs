import React from 'react'
import withNavigateHook from './nagivateHook'
import {
    IconEye, IconEyeOff, IconCopy, IconDownload, IconCheck, IconPencil,
} from './Icons'

class NodeViewer extends React.Component {
    constructor(props) {
        super(props)
        this.state = {
            revealed: {},  // field name → revealed plaintext
            copied:   null, // field name showing copy confirmation
        }
        this._hideTimers = {}
        this._copyTimer  = null
    }

    // the reveal state belongs to the entry it was requested for: without this
    // a password revealed on one entry stays on screen when the next one is
    // opened into the same panel
    componentDidUpdate(prevProps) {
        const previous = prevProps.entry && prevProps.entry.id
        const current  = this.props.entry && this.props.entry.id
        if (previous === current) return

        Object.values(this._hideTimers).forEach(clearTimeout)
        this._hideTimers = {}
        clearTimeout(this._copyTimer)
        this.setState({ revealed: {}, copied: null })
    }

    componentWillUnmount() {
        if (this.serverRequest) this.serverRequest.abort()
        Object.values(this._hideTimers).forEach(clearTimeout)
        clearTimeout(this._copyTimer)
    }

    // ── Protected field reveal / hide ─────────────────────────────

    revealField(name) {
        this.serverRequest = KeePass4Web.fetch('get_protected', {
            method: 'GET',
            data: { entry_id: this.props.entry.id, name },
            success: (data) => {
                this.setState(prev => ({ revealed: { ...prev.revealed, [name]: data ?? '' } }))
                clearTimeout(this._hideTimers[name])
                this._hideTimers[name] = setTimeout(
                    () => this.hideField(name),
                    this.props.timeoutSec || 30000,
                )
            },
            error: KeePass4Web.error.bind(this),
        })
    }

    hideField(name) {
        this.setState(prev => {
            const r = { ...prev.revealed }
            delete r[name]
            return { revealed: r }
        })
    }

    toggleField(name) {
        if (Object.prototype.hasOwnProperty.call(this.state.revealed, name)) {
            this.hideField(name)
        } else {
            this.revealField(name)
        }
    }

    // ── Copy helpers ──────────────────────────────────────────────

    copyValue(value, name) {
        if (value == null) value = ''
        navigator.clipboard.writeText(value)
            .then(() => {
                this.setState({ copied: name })
                clearTimeout(this._copyTimer)
                this._copyTimer = setTimeout(() => this.setState({ copied: null }), 1500)
            })
            .catch(() => {})
    }

    copyProtected(name) {
        this.serverRequest = KeePass4Web.fetch('get_protected', {
            method: 'GET',
            data: { entry_id: this.props.entry.id, name },
            success: (data) => this.copyValue(data, name),
            error: KeePass4Web.error.bind(this),
        })
    }

    // ── File download ─────────────────────────────────────────────

    downloadFile(filename) {
        const xhr = new XMLHttpRequest()
        xhr.open('GET', 'get_file', true)
        xhr.responseType = 'arraybuffer'
        xhr.setRequestHeader('X-CSRF-Token', KeePass4Web.getCSRFToken())
        xhr.setRequestHeader('Content-type', 'application/x-www-form-urlencoded; charset=UTF-8')
        xhr.setRequestHeader('X-Requested-With', 'XMLHttpRequest')
        xhr.onload = function () {
            if (xhr.status === 200) {
                let name = ''
                const disp = xhr.getResponseHeader('Content-Disposition')
                if (disp && disp.indexOf('attachment') !== -1) {
                    const m = /filename[^;=\n]*=((['"]).*?\2|[^;\n]*)/.exec(disp)
                    if (m && m[1]) name = decodeURIComponent(m[1].replace(/['"]/g, '').replace(/^UTF-8/i, ''))
                }
                const type = xhr.getResponseHeader('Content-Type')
                const blob = new Blob([xhr.response], { type })
                const URL  = window.URL || window.webkitURL
                const url  = URL.createObjectURL(blob)
                if (name) {
                    const a  = document.createElement('a')
                    a.href   = url
                    a.download = name
                    document.body.appendChild(a)
                    a.click()
                    document.body.removeChild(a)
                } else {
                    window.location = url
                }
                setTimeout(() => URL.revokeObjectURL(url), 100)
            } else if (xhr.status >= 400) {
                KeePass4Web.error(xhr, null, xhr.responseText)
            }
        }
        KeePass4Web.restartTimer(true)
        xhr.send('id=' + encodeURIComponent(this.props.entry.id) + '&filename=' + encodeURIComponent(filename))
    }

    // ── Render helpers ────────────────────────────────────────────

    renderCopyBtn(value, name) {
        const isCopied = this.state.copied === name
        return (
            <button
                className="kp-btn-icon"
                data-testid="copy"
                title={isCopied ? 'Copied!' : 'Copy'}
                onClick={() => this.copyValue(value, name)}
            >
                {isCopied ? <IconCheck size={13}/> : <IconCopy size={13}/>}
            </button>
        )
    }

    renderProtectedField(label, name) {
        const { revealed, copied } = this.state
        const isRevealed = Object.prototype.hasOwnProperty.call(revealed, name)
        const display    = isRevealed ? (revealed[name] || '(empty)') : '••••••••'

        return (
            <div className="kp-node-field" data-testid="entry-field" key={name}>
                <span className="kp-node-field-label" data-testid="entry-field-label">{label}</span>
                <span
                    className={`kp-node-field-value${isRevealed ? '' : ' protected'}`}
                    data-testid="entry-field-value"
                    data-revealed={isRevealed ? 'true' : 'false'}
                >
                    {display}
                </span>
                <span className="kp-node-field-actions">
                    <button
                        className="kp-btn-icon"
                        data-testid="reveal"
                        data-revealed={isRevealed ? 'true' : 'false'}
                        title={isRevealed ? 'Hide' : 'Show'}
                        onClick={() => this.toggleField(name)}
                    >
                        {isRevealed ? <IconEyeOff size={13}/> : <IconEye size={13}/>}
                    </button>
                    <button
                        className="kp-btn-icon"
                        data-testid="copy"
                        title={copied === name ? 'Copied!' : 'Copy'}
                        onClick={() => this.copyProtected(name)}
                    >
                        {copied === name ? <IconCheck size={13}/> : <IconCopy size={13}/>}
                    </button>
                </span>
            </div>
        )
    }

    render() {
        const { entry, mask } = this.props

        if (!entry) return <div className={`kp-detail-content${mask ? ' kp-loading' : ''}`}/>

        let icon = null
        if (entry.custom_icon_uuid) {
            icon = <img className="kp-icon" style={{ width: 36, height: 36, objectFit: 'contain', borderRadius: 8 }}
                        src={'api/v1/icon/' + encodeURIComponent(entry.custom_icon_uuid)} alt=""/>
        } else if (entry.icon) {
            icon = <img className="kp-icon" style={{ width: 36, height: 36, objectFit: 'contain', borderRadius: 8 }}
                        src={'assets/img/icons/' + encodeURIComponent(entry.icon) + '.png'} alt=""/>
        }

        const tags = (entry.tags || []).map(t => (
            <span key={t} className="kp-badge">{t}</span>
        ))

        const extraFields = []
        const strings = entry.strings || {}
        for (const name of Object.keys(strings)) {
            if (entry.protected && Object.prototype.hasOwnProperty.call(entry.protected, name)) {
                extraFields.push(this.renderProtectedField(name, name))
            } else {
                extraFields.push(
                    <div className="kp-node-field" data-testid="entry-field" key={name}>
                        <span className="kp-node-field-label" data-testid="entry-field-label">{name}</span>
                        <span className="kp-node-field-value" data-testid="entry-field-value">{strings[name]}</span>
                        <span className="kp-node-field-actions">
                            {this.renderCopyBtn(strings[name], name)}
                        </span>
                    </div>
                )
            }
        }

        const files = []
        for (const fname of Object.keys(entry.binary || {})) {
            files.push(
                <div className="kp-node-field" data-testid="entry-field" key={fname}>
                    <span className="kp-node-field-label" data-testid="entry-field-label">File</span>
                    <span className="kp-node-field-value" data-testid="entry-field-value">{fname}</span>
                    <span className="kp-node-field-actions">
                        <button
                            className="kp-btn-icon"
                            title="Download"
                            onClick={() => this.downloadFile(fname)}
                        >
                            <IconDownload size={13}/>
                        </button>
                    </span>
                </div>
            )
        }

        return (
            <div className={`kp-detail-content${mask ? ' kp-loading' : ''}`}>
                <div className="kp-detail-header">
                    <h3>Entry Details</h3>
                    {this.props.onEdit && (
                        <button className="kp-btn kp-btn-ghost kp-btn-sm" onClick={this.props.onEdit}>
                            <IconPencil size={13}/> Edit
                        </button>
                    )}
                </div>

                <div className="kp-detail-entry-meta">
                    <div className="kp-detail-icon">
                        {icon || <span style={{ fontSize: 28 }}>🔑</span>}
                    </div>
                    <div className="kp-detail-icon-info">
                        <h4 data-testid="entry-title">{entry.title}</h4>
                        {entry.url && (
                            <p>
                                <a href={entry.url} target="_blank" rel="noopener noreferrer">{entry.url}</a>
                            </p>
                        )}
                    </div>
                </div>

                <div className="kp-node-fields">
                    <div className="kp-node-field" data-testid="entry-field">
                        <span className="kp-node-field-label" data-testid="entry-field-label">Username</span>
                        <span className="kp-node-field-value" data-testid="entry-field-value">{entry.username || '—'}</span>
                        <span className="kp-node-field-actions">
                            {this.renderCopyBtn(entry.username, 'username')}
                        </span>
                    </div>

                    {this.renderProtectedField('Password', 'password')}

                    {entry.url && (
                        <div className="kp-node-field" data-testid="entry-field">
                            <span className="kp-node-field-label" data-testid="entry-field-label">URL</span>
                            <span className="kp-node-field-value" data-testid="entry-field-value">
                                <a href={entry.url} target="_blank" rel="noopener noreferrer">{entry.url}</a>
                            </span>
                            <span className="kp-node-field-actions">
                                {this.renderCopyBtn(entry.url, 'url')}
                            </span>
                        </div>
                    )}

                    {entry.notes && (
                        <div className="kp-node-field" data-testid="entry-field" style={{ alignItems: 'flex-start' }}>
                            <span className="kp-node-field-label" data-testid="entry-field-label">Notes</span>
                            <span className="kp-node-field-value" data-testid="entry-field-value" style={{ whiteSpace: 'pre-wrap' }}>
                                {entry.notes}
                            </span>
                            <span className="kp-node-field-actions"/>
                        </div>
                    )}

                    {tags.length > 0 && (
                        <div className="kp-node-field" data-testid="entry-field">
                            <span className="kp-node-field-label" data-testid="entry-field-label">Tags</span>
                            <span className="kp-node-field-value" data-testid="entry-field-value" style={{ display: 'flex', gap: 4, flexWrap: 'wrap' }}>
                                {tags}
                            </span>
                            <span className="kp-node-field-actions"/>
                        </div>
                    )}

                    {extraFields.length > 0 && (
                        <>
                            <div className="kp-node-section-label">Custom Fields</div>
                            {extraFields}
                        </>
                    )}

                    {files.length > 0 && (
                        <>
                            <div className="kp-node-section-label">Attachments</div>
                            {files}
                        </>
                    )}
                </div>
            </div>
        )
    }
}

export default withNavigateHook(NodeViewer)
