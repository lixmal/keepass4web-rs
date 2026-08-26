import React from 'react'
import { IconEye, IconEyeOff, IconRefresh, IconSave } from './Icons'

const ICON_COUNT = 69  // KeePass standard icons: 0–68

function generatePassword(length = 20) {
    const charset = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()-_=+[]{}|;:,.<>?'
    const arr = new Uint8Array(length)
    window.crypto.getRandomValues(arr)
    return Array.from(arr).map(b => charset[b % charset.length]).join('')
}

function iconSrc(id) {
    return `img/icons/${id}.png`
}

class EntryForm extends React.Component {
    constructor(props) {
        super(props)
        const e = props.entry || {}
        this.state = {
            form: {
                title:    e.title    || '',
                username: e.username || '',
                password: '',
                url:      e.url      || '',
                notes:    '',
                icon:     e.icon     != null ? e.icon : null,
            },
            saving:       false,
            showPassword: false,
            pickerOpen:   false,
        }
        this._closePickerOutside = this._closePickerOutside.bind(this)
    }

    componentDidUpdate(prev) {
        if (prev.entry !== this.props.entry || prev.mode !== this.props.mode) {
            const e = this.props.entry || {}
            this.setState({
                form: {
                    title:    e.title    || '',
                    username: e.username || '',
                    password: '',
                    url:      e.url      || '',
                    notes:    '',
                    icon:     e.icon     != null ? e.icon : null,
                },
                saving: false,
                showPassword: false,
                pickerOpen: false,
            })
        }
    }

    componentWillUnmount() {
        document.removeEventListener('mousedown', this._closePickerOutside)
    }

    _closePickerOutside(e) {
        if (this._pickerRef && !this._pickerRef.contains(e.target)) {
            this.closePicker()
        }
    }

    openPicker() {
        this.setState({ pickerOpen: true })
        document.addEventListener('mousedown', this._closePickerOutside)
    }

    closePicker() {
        this.setState({ pickerOpen: false })
        document.removeEventListener('mousedown', this._closePickerOutside)
    }

    selectIcon(id) {
        this.setState(prev => ({ form: { ...prev.form, icon: id }, pickerOpen: false }))
        document.removeEventListener('mousedown', this._closePickerOutside)
    }

    set(field, e) {
        const val = e.target.value
        this.setState(prev => ({ form: { ...prev.form, [field]: val } }))
    }

    genPassword() {
        this.setState(prev => ({
            form: { ...prev.form, password: generatePassword() },
            showPassword: true,
        }))
    }

    submit(e) {
        e.preventDefault()
        const { mode, entry, group } = this.props
        const { form } = this.state
        if (!group && mode === 'new') return

        this.setState({ saving: true })

        const payload = { ...form }
        if (payload.icon === null) delete payload.icon

        if (mode === 'new') {
            KeePass4Web.fetch('entry', {
                method: 'POST',
                data: { group_id: group.id, ...payload },
                success: (data) => {
                    this.setState({ saving: false })
                    if (this.props.onSaved) this.props.onSaved(data && data.data && data.data.id)
                },
                error: (err) => {
                    this.setState({ saving: false })
                    KeePass4Web.error.call(this, err)
                },
            })
        } else {
            KeePass4Web.fetch('entry', {
                method: 'PUT',
                data: { id: entry.id, ...payload },
                success: () => {
                    this.setState({ saving: false })
                    if (this.props.onSaved) this.props.onSaved(entry.id)
                },
                error: (err) => {
                    this.setState({ saving: false })
                    KeePass4Web.error.call(this, err)
                },
            })
        }
    }

    render() {
        const { mode, entry, group, onCancel } = this.props
        const { form, saving, showPassword, pickerOpen } = this.state
        const isNew = mode === 'new'

        const groupName  = group ? group.title : ''
        const entryName  = isNew ? 'New Entry' : (entry && entry.title) || 'Entry'
        const selectedIcon = form.icon != null ? form.icon : 0

        // icon picker grid
        const iconGrid = []
        for (let i = 0; i < ICON_COUNT; i++) {
            iconGrid.push(
                <div
                    key={i}
                    className={`kp-icon-grid-item${form.icon === i ? ' selected' : ''}`}
                    title={`Icon ${i}`}
                    onClick={() => this.selectIcon(i)}
                >
                    <img src={iconSrc(i)} alt={`icon ${i}`}/>
                </div>
            )
        }

        return (
            <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
                <div className="kp-detail-header">
                    <h3>{isNew ? 'New Entry' : 'Edit Entry'}</h3>
                </div>

                <div className="kp-detail-entry-meta">
                    <div className="kp-detail-icon" style={{ background: 'none', border: '1.5px solid var(--kp-border)' }}>
                        <img src={iconSrc(selectedIcon)} style={{ width: 24, height: 24, objectFit: 'contain' }} alt=""/>
                    </div>
                    <div className="kp-detail-icon-info">
                        <h4>{entryName}</h4>
                        <p>{groupName}</p>
                    </div>
                    {!isNew && <span className="kp-detail-modified">Editing</span>}
                </div>

                <form className="kp-form" onSubmit={this.submit.bind(this)}>

                    {/* Icon + Title row */}
                    <div className="kp-form-icon-row">
                        <div
                            className="kp-icon-picker-wrap"
                            ref={r => (this._pickerRef = r)}
                        >
                            <button
                                type="button"
                                className="kp-icon-trigger"
                                title="Change icon"
                                onClick={() => pickerOpen ? this.closePicker() : this.openPicker()}
                            >
                                <img src={iconSrc(selectedIcon)} alt="entry icon"/>
                            </button>

                            {pickerOpen && (
                                <div className="kp-icon-picker">
                                    <div className="kp-icon-picker-label">Choose icon</div>
                                    <div className="kp-icon-grid">{iconGrid}</div>
                                </div>
                            )}
                        </div>

                        <div className="kp-field" style={{ flex: 1 }}>
                            <label htmlFor="kp-f-title">Title</label>
                            <input
                                className="kp-input" id="kp-f-title"
                                type="text" placeholder="Entry title" required autoFocus
                                value={form.title}
                                onChange={this.set.bind(this, 'title')}
                            />
                        </div>
                    </div>

                    <div className="kp-field">
                        <label htmlFor="kp-f-username">Username</label>
                        <input
                            className="kp-input" id="kp-f-username"
                            type="text" placeholder="Username"
                            value={form.username}
                            onChange={this.set.bind(this, 'username')}
                        />
                    </div>

                    <div className="kp-field">
                        <label htmlFor="kp-f-password">Password</label>
                        <div className="kp-input-group">
                            <input
                                className="kp-input" id="kp-f-password"
                                type={showPassword ? 'text' : 'password'}
                                placeholder={isNew ? 'Password' : 'Leave blank to keep existing'}
                                value={form.password}
                                onChange={this.set.bind(this, 'password')}
                            />
                            <div className="kp-input-group-btns">
                                <button
                                    type="button" className="kp-btn-outline kp-btn"
                                    title={showPassword ? 'Hide' : 'Show'}
                                    onClick={() => this.setState(p => ({ showPassword: !p.showPassword }))}
                                >
                                    {showPassword ? <IconEyeOff size={14}/> : <IconEye size={14}/>}
                                </button>
                                <button
                                    type="button" className="kp-btn-outline kp-btn"
                                    title="Generate random password"
                                    onClick={this.genPassword.bind(this)}
                                >
                                    <IconRefresh size={14}/>
                                </button>
                            </div>
                        </div>
                    </div>

                    <div className="kp-field">
                        <label htmlFor="kp-f-url">URL</label>
                        <input
                            className="kp-input" id="kp-f-url"
                            type="text" placeholder="https://…"
                            value={form.url}
                            onChange={this.set.bind(this, 'url')}
                        />
                    </div>

                    <div className="kp-field">
                        <label htmlFor="kp-f-notes">Notes</label>
                        <textarea
                            className="kp-input" id="kp-f-notes"
                            placeholder="Optional notes…"
                            value={form.notes}
                            onChange={this.set.bind(this, 'notes')}
                        />
                    </div>

                    <div className="kp-form-actions">
                        <button type="button" className="kp-btn kp-btn-outline" onClick={onCancel}>
                            Cancel
                        </button>
                        <button type="submit" className="kp-btn kp-btn-primary" disabled={saving}>
                            <IconSave size={13}/>
                            {saving ? 'Saving…' : 'Save Entry'}
                        </button>
                    </div>
                </form>
            </div>
        )
    }
}

export default EntryForm
