import React from 'react'
import { IconEye, IconEyeOff, IconLock, IconPlus, IconRefresh, IconSave, IconTrash } from './Icons'

const ICON_COUNT = 69  // KeePass standard icons: 0–68

function generatePassword(length = 20) {
    const charset = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()-_=+[]{}|;:,.<>?'
    // taking a byte modulo the alphabet would favour its first characters,
    // since 256 is not a multiple of the alphabet length: draw again instead
    const limit = Math.floor(256 / charset.length) * charset.length
    const password = []

    while (password.length < length) {
        const bytes = new Uint8Array(length - password.length)
        window.crypto.getRandomValues(bytes)
        for (const byte of bytes) {
            if (byte < limit) password.push(charset[byte % charset.length])
        }
    }

    return password.join('')
}

function iconSrc(id) {
    return `assets/img/icons/${id}.png`
}

// The entry lists its custom fields with the protected ones blanked out, since
// their values are only handed over when they are asked for one at a time. An
// empty protected field left as it is keeps the value it has on the server.
function customFieldsOf(entry) {
    const strings = (entry && entry.strings) || {}
    const protectedFields = (entry && entry.protected) || {}

    return Object.keys(strings).map(name => ({
        name,
        value: strings[name] == null ? '' : strings[name],
        protected: Object.prototype.hasOwnProperty.call(protectedFields, name),
    }))
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
                notes:    e.notes    || '',
                icon:     e.icon     != null ? e.icon : null,
                tags:     (e.tags || []).join(', '),
                fields:   customFieldsOf(e),
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
                    notes:    e.notes    || '',
                    icon:     e.icon     != null ? e.icon : null,
                    tags:     (e.tags || []).join(', '),
                    fields:   customFieldsOf(e),
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

    setField(index, key, e) {
        const val = key === 'protected' ? e.target.checked : e.target.value
        this.setState(prev => ({
            form: {
                ...prev.form,
                fields: prev.form.fields.map((field, i) => (
                    i === index ? { ...field, [key]: val } : field
                )),
            },
        }))
    }

    addField() {
        this.setState(prev => ({
            form: { ...prev.form, fields: [...prev.form.fields, { name: '', value: '', protected: false }] },
        }))
    }

    removeField(index) {
        this.setState(prev => ({
            form: { ...prev.form, fields: prev.form.fields.filter((_, i) => i !== index) },
        }))
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
        // a form encoded body cannot carry a list, so the custom fields travel
        // as json, and a field without a name is one the user never filled in
        payload.fields = JSON.stringify(
            form.fields.filter(field => field.name.trim() !== ''),
        )

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

                    <div className="kp-field">
                        <label htmlFor="kp-f-tags">Tags</label>
                        <input
                            className="kp-input" id="kp-f-tags"
                            type="text" placeholder="Comma separated"
                            value={form.tags}
                            onChange={this.set.bind(this, 'tags')}
                        />
                    </div>

                    <div className="kp-field">
                        <label>Custom fields</label>
                        {form.fields.map((field, i) => (
                            <div className="kp-form-field-row" key={i} data-testid="custom-field-row">
                                <input
                                    className="kp-input"
                                    type="text" placeholder="Name"
                                    data-testid="custom-field-name"
                                    value={field.name}
                                    onChange={this.setField.bind(this, i, 'name')}
                                />
                                <input
                                    className="kp-input"
                                    type={field.protected ? 'password' : 'text'}
                                    placeholder={field.protected && field.value === '' ? 'Unchanged' : 'Value'}
                                    data-testid="custom-field-value"
                                    value={field.value}
                                    onChange={this.setField.bind(this, i, 'value')}
                                />
                                <label className="kp-form-field-protected" title="Store the value protected">
                                    <input
                                        type="checkbox"
                                        data-testid="custom-field-protected"
                                        checked={field.protected}
                                        onChange={this.setField.bind(this, i, 'protected')}
                                    />
                                    <IconLock size={13}/>
                                </label>
                                <button
                                    type="button"
                                    className="kp-btn kp-btn-ghost"
                                    title="Remove this field"
                                    data-testid="custom-field-remove"
                                    onClick={this.removeField.bind(this, i)}
                                >
                                    <IconTrash size={13}/>
                                </button>
                            </div>
                        ))}
                        <button
                            type="button"
                            className="kp-btn kp-btn-outline"
                            data-testid="custom-field-add"
                            onClick={this.addField.bind(this)}
                        >
                            <IconPlus size={13}/>
                            Add field
                        </button>
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
