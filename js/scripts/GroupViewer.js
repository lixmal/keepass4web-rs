import React from 'react'
import Classnames from 'classnames'

import withNavigateHook from './nagivateHook'

const NIL_UUID = '00000000-0000-0000-0000-000000000000'

function generatePassword(length = 20) {
    const charset = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()-_=+[]{}|;:,.<>?'
    const arr = new Uint8Array(length)
    window.crypto.getRandomValues(arr)
    return Array.from(arr).map(b => charset[b % charset.length]).join('')
}

const EMPTY_FORM = { title: '', username: '', password: '', url: '', notes: '' }

class GroupViewer extends React.Component {
    constructor(props) {
        super(props)
        this.state = {
            // new entry form
            showForm: false,
            form: { ...EMPTY_FORM },
            saving: false,
            showPassword: false,
            // edit entry
            editEntry: null,
            editForm: { ...EMPTY_FORM },
            editSaving: false,
            editShowPassword: false,
            // rename group
            editingTitle: false,
            titleDraft: '',
            titleSaving: false,
            // new group
            showGroupForm: false,
            newGroupName: '',
            groupSaving: false,
        }
    }

    getIcon(element) {
        if (element.custom_icon_uuid)
            return <img className="kp-icon" src={'api/v1/icon/' + encodeURIComponent(element.custom_icon_uuid)}/>
        else if (element.icon)
            return <img className="kp-icon" src={'assets/img/icons/' + encodeURIComponent(element.icon) + '.png'}/>
    }

    // ── new entry ────────────────────────────────────────────────────────────

    onNewEntry() {
        this.setState({ showForm: true, showPassword: false, form: { ...EMPTY_FORM }, editEntry: null })
    }

    onCancel() {
        this.setState({ showForm: false })
    }

    onFormChange(field, e) {
        this.setState(prev => ({ form: { ...prev.form, [field]: e.target.value } }))
    }

    onGeneratePassword() {
        this.setState(prev => ({ form: { ...prev.form, password: generatePassword() }, showPassword: true }))
    }

    onSubmit(e) {
        e.preventDefault()
        if (!this.props.group) return
        this.setState({ saving: true })
        KeePass4Web.fetch('entry', {
            method: 'POST',
            data: { group_id: this.props.group.id, ...this.state.form },
            success: (data) => {
                this.setState({ showForm: false, saving: false })
                if (this.props.onEntryCreated) this.props.onEntryCreated(data && data.id)
            },
            error: (err) => {
                this.setState({ saving: false })
                KeePass4Web.error.call(this, err)
            },
        })
    }

    // ── edit entry ───────────────────────────────────────────────────────────

    onEditEntry(entry, ev) {
        ev.stopPropagation()
        this.setState({
            editEntry: entry,
            editForm: { title: entry.title || '', username: entry.username || '', password: '', url: entry.url || '', notes: '' },
            editShowPassword: false,
            showForm: false,
        })
    }

    onEditFormChange(field, e) {
        this.setState(prev => ({ editForm: { ...prev.editForm, [field]: e.target.value } }))
    }

    onEditGeneratePassword() {
        this.setState(prev => ({ editForm: { ...prev.editForm, password: generatePassword() }, editShowPassword: true }))
    }

    onEditSubmit(e) {
        e.preventDefault()
        const { editEntry, editForm } = this.state
        this.setState({ editSaving: true })
        KeePass4Web.fetch('entry', {
            method: 'PUT',
            data: { id: editEntry.id, ...editForm },
            success: () => {
                this.setState({ editEntry: null, editSaving: false })
                if (this.props.onEntryCreated) this.props.onEntryCreated()
            },
            error: (err) => {
                this.setState({ editSaving: false })
                KeePass4Web.error.call(this, err)
            },
        })
    }

    // ── create group ─────────────────────────────────────────────────────────

    onNewGroup() {
        this.setState({ showGroupForm: true, newGroupName: '', showForm: false, editEntry: null })
    }

    onGroupFormSubmit(e) {
        e.preventDefault()
        if (!this.props.group || !this.state.newGroupName.trim()) return
        this.setState({ groupSaving: true })
        KeePass4Web.fetch('group', {
            method: 'POST',
            data: { parent_id: this.props.group.id, title: this.state.newGroupName.trim() },
            success: () => {
                this.setState({ showGroupForm: false, newGroupName: '', groupSaving: false })
                if (this.props.onGroupCreated) this.props.onGroupCreated()
            },
            error: (err) => {
                this.setState({ groupSaving: false })
                KeePass4Web.error.call(this, err)
            },
        })
    }

    // ── rename group ─────────────────────────────────────────────────────────

    onRenameStart() {
        this.setState({ editingTitle: true, titleDraft: this.props.group.title })
    }

    onRenameSave(e) {
        e.preventDefault()
        const { titleDraft } = this.state
        if (!titleDraft.trim()) return
        this.setState({ titleSaving: true })
        KeePass4Web.fetch('group', {
            method: 'PUT',
            data: { id: this.props.group.id, title: titleDraft.trim() },
            success: () => {
                this.setState({ editingTitle: false, titleSaving: false })
                if (this.props.onGroupRenamed) this.props.onGroupRenamed(this.props.group.id, titleDraft.trim())
            },
            error: (err) => {
                this.setState({ titleSaving: false })
                KeePass4Web.error.call(this, err)
            },
        })
    }

    // ── render helpers ───────────────────────────────────────────────────────

    renderPasswordInput(value, showField, onChangeFn, onToggleFn, onGenFn) {
        const eyeIcon = showField ? 'glyphicon-eye-close' : 'glyphicon-eye-open'
        return (
            <div className="input-group">
                <input
                    className="form-control"
                    type={showField ? 'text' : 'password'}
                    placeholder="Password (leave blank to keep existing)"
                    value={value}
                    onChange={onChangeFn}
                />
                <div className="input-group-btn">
                    <button type="button" className="btn btn-default btn-sm" title="Show / hide" onClick={onToggleFn}>
                        <span className={'glyphicon ' + eyeIcon}></span>
                    </button>
                    <button type="button" className="btn btn-default btn-sm" title="Generate random password" onClick={onGenFn}>
                        <span className="glyphicon glyphicon-refresh"></span>
                    </button>
                </div>
            </div>
        )
    }

    renderEntryForm(form, showPw, onChangeFn, onTogglePw, onGenPw, onSubmitFn, onCancelFn, saving) {
        return (
            <tr key="entry-form">
                <td colSpan="3">
                    <form onSubmit={onSubmitFn}>
                        <div className="form-group form-group-sm">
                            <input className="form-control" placeholder="Title" required autoFocus
                                value={form.title} onChange={onChangeFn.bind(this, 'title')} />
                        </div>
                        <div className="form-group form-group-sm">
                            <input className="form-control" placeholder="Username"
                                value={form.username} onChange={onChangeFn.bind(this, 'username')} />
                        </div>
                        <div className="form-group form-group-sm">
                            {this.renderPasswordInput(
                                form.password, showPw,
                                onChangeFn.bind(this, 'password'),
                                onTogglePw, onGenPw
                            )}
                        </div>
                        <div className="form-group form-group-sm">
                            <input className="form-control" placeholder="URL"
                                value={form.url} onChange={onChangeFn.bind(this, 'url')} />
                        </div>
                        <div className="form-group form-group-sm">
                            <input className="form-control" placeholder="Notes"
                                value={form.notes} onChange={onChangeFn.bind(this, 'notes')} />
                        </div>
                        <div className="btn-group">
                            <button type="submit" className="btn btn-primary btn-sm" disabled={saving}>
                                {saving ? 'Saving…' : 'Save'}
                            </button>
                            <button type="button" className="btn btn-default btn-sm" onClick={onCancelFn}>
                                Cancel
                            </button>
                        </div>
                    </form>
                </td>
            </tr>
        )
    }

    render() {
        const classes = Classnames({ 'panel': true, 'panel-default': true, 'loading-mask': this.props.mask })

        if (!this.props.group) return (<div className={classes}></div>)

        const group = this.props.group
        const isSearchResult = group.id === NIL_UUID
        const canAddGroup = !isSearchResult && (this.props.groupDepth === undefined || this.props.groupDepth < 2)
        const { editEntry, editingTitle, titleDraft, titleSaving, showGroupForm, newGroupName, groupSaving } = this.state

        // ── group heading ────────────────────────────────────────────────────
        let heading
        if (editingTitle) {
            heading = (
                <form className="form-inline" onSubmit={this.onRenameSave.bind(this)}
                      style={{ display: 'inline' }}>
                    <div className="input-group input-group-sm" style={{ maxWidth: 260 }}>
                        <input
                            className="form-control"
                            value={titleDraft}
                            autoFocus
                            onChange={e => this.setState({ titleDraft: e.target.value })}
                        />
                        <div className="input-group-btn">
                            <button type="submit" className="btn btn-primary btn-sm" disabled={titleSaving}>
                                {titleSaving ? '…' : <span className="glyphicon glyphicon-ok"></span>}
                            </button>
                            <button type="button" className="btn btn-default btn-sm"
                                onClick={() => this.setState({ editingTitle: false })}>
                                <span className="glyphicon glyphicon-remove"></span>
                            </button>
                        </div>
                    </div>
                </form>
            )
        } else {
            heading = (
                <span>
                    {this.getIcon(group)}
                    {group.title}
                    {!isSearchResult && (
                        <button className="btn btn-link btn-xs" style={{ padding: '0 4px' }}
                            title="Rename group" onClick={this.onRenameStart.bind(this)}>
                            <span className="glyphicon glyphicon-pencil"></span>
                        </button>
                    )}
                </span>
            )
        }

        // ── entry rows ───────────────────────────────────────────────────────
        let entries = []
        for (var i in group.entries) {
            let entry = group.entries[i]
            const isEditing = editEntry && editEntry.id === entry.id

            entries.push(
                <tr key={entry.id} onClick={this.props.onSelect.bind(this, entry)}
                    className={isEditing ? 'active' : ''}>
                    <td className="kp-wrap" data-testid="entry-row-title">
                        {this.getIcon(entry)}
                        {entry.title}
                    </td>
                    <td className="kp-wrap" data-testid="entry-row-username">{entry.username}</td>
                    <td style={{ whiteSpace: 'nowrap' }}>
                        <button className="btn btn-default btn-xs" title="Edit entry"
                            onClick={this.onEditEntry.bind(this, entry)}>
                            <span className="glyphicon glyphicon-pencil"></span>
                        </button>
                        {' '}
                        <button className="btn btn-danger btn-xs" title="Delete entry"
                            onClick={(ev) => { ev.stopPropagation(); this.props.onDeleteEntry && this.props.onDeleteEntry(entry) }}>
                            <span className="glyphicon glyphicon-trash"></span>
                        </button>
                    </td>
                </tr>
            )

            if (isEditing) {
                entries.push(this.renderEntryForm(
                    this.state.editForm,
                    this.state.editShowPassword,
                    this.onEditFormChange.bind(this),
                    () => this.setState(p => ({ editShowPassword: !p.editShowPassword })),
                    this.onEditGeneratePassword.bind(this),
                    this.onEditSubmit.bind(this),
                    () => this.setState({ editEntry: null }),
                    this.state.editSaving,
                ))
            }
        }

        // ── new entry form ───────────────────────────────────────────────────
        let newEntryForm = null
        if (this.state.showForm) {
            newEntryForm = this.renderEntryForm(
                this.state.form,
                this.state.showPassword,
                this.onFormChange.bind(this),
                () => this.setState(p => ({ showPassword: !p.showPassword })),
                this.onGeneratePassword.bind(this),
                this.onSubmit.bind(this),
                this.onCancel.bind(this),
                this.state.saving,
            )
        }

        return (
            <div className={classes}>
                <div className="panel-heading" style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                    <span data-testid="group-title">{heading}</span>
                    {!isSearchResult && !editingTitle && (
                        <span>
                            {canAddGroup && (
                                <button className="btn btn-info btn-xs" style={{ marginRight: 4 }}
                                    onClick={this.onNewGroup.bind(this)} title="Add subgroup">
                                    <span className="glyphicon glyphicon-folder-open"></span> Add Group
                                </button>
                            )}
                            <button className="btn btn-success btn-xs"
                                onClick={this.onNewEntry.bind(this)} title="New entry">
                                <span className="glyphicon glyphicon-plus"></span> New Entry
                            </button>
                        </span>
                    )}
                </div>
                {showGroupForm && (
                    <div className="panel-body" style={{ paddingTop: 8, paddingBottom: 8, borderBottom: '1px solid #ddd' }}>
                        <form className="form-inline" onSubmit={this.onGroupFormSubmit.bind(this)}>
                            <div className="input-group input-group-sm" style={{ maxWidth: 300 }}>
                                <input
                                    className="form-control"
                                    placeholder="Group name"
                                    required
                                    autoFocus
                                    value={newGroupName}
                                    onChange={e => this.setState({ newGroupName: e.target.value })}
                                />
                                <div className="input-group-btn">
                                    <button type="submit" className="btn btn-primary btn-sm" disabled={groupSaving}>
                                        {groupSaving ? '…' : 'Create'}
                                    </button>
                                    <button type="button" className="btn btn-default btn-sm"
                                        onClick={() => this.setState({ showGroupForm: false })}>
                                        Cancel
                                    </button>
                                </div>
                            </div>
                        </form>
                    </div>
                )}
                <div className="panel-body">
                    <table className="table table-hover table-condensed kp-table">
                        <thead>
                        <tr>
                            <th>Entry Name</th>
                            <th>Username</th>
                            <th></th>
                        </tr>
                        </thead>
                        <tbody className="groupview-body">
                        {entries}
                        {newEntryForm}
                        </tbody>
                    </table>
                </div>
            </div>
        )
    }
}

export default withNavigateHook(GroupViewer)
