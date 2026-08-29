import React from 'react'
import withNavigateHook from './nagivateHook'
import GroupPicker from './GroupPicker'
import {
    IconPlus, IconPencil, IconTrash, IconLock, IconCheck, IconX, IconFolder, IconMonitor, IconDatabase,
    IconMove,
} from './Icons'

const NIL_UUID = '00000000-0000-0000-0000-000000000000'

// an entry past the date it was set to expire on
function isExpired(entry) {
    return Boolean(entry.expires && entry.expiry && new Date(entry.expiry) < new Date())
}

function entryIcon(entry) {
    if (entry.custom_icon_uuid)
        return <img className="kp-icon" style={{ width: 18, height: 18, objectFit: 'contain' }}
                    src={'api/v1/icon/' + encodeURIComponent(entry.custom_icon_uuid)} alt=""/>
    if (entry.url && entry.url.includes('db'))
        return <IconDatabase size={18}/>
    return <IconMonitor size={18}/>
}

class GroupViewer extends React.Component {
    constructor(props) {
        super(props)
        this.state = {
            // group rename
            editingTitle:  false,
            titleDraft:    '',
            titleSaving:   false,
            // add subgroup
            showGroupForm: false,
            newGroupName:  '',
            groupSaving:   false,
            // the entry or group waiting for a destination
            moving:        null,
        }
    }

    // ── Group rename ─────────────────────────────────────────────

    startRename() {
        this.setState({ editingTitle: true, titleDraft: this.props.group.title })
    }

    saveRename(e) {
        e.preventDefault()
        const name = this.state.titleDraft.trim()
        if (!name) return
        this.setState({ titleSaving: true })
        KeePass4Web.fetch('group', {
            method: 'PUT',
            data: { id: this.props.group.id, title: name },
            success: () => {
                this.setState({ editingTitle: false, titleSaving: false })
                if (this.props.onGroupRenamed) this.props.onGroupRenamed(this.props.group.id, name)
            },
            error: (err) => {
                this.setState({ titleSaving: false })
                KeePass4Web.error.call(this, err)
            },
        })
    }

    // ── Create subgroup ──────────────────────────────────────────

    saveGroup(e) {
        e.preventDefault()
        const name = this.state.newGroupName.trim()
        if (!name) return
        this.setState({ groupSaving: true })
        KeePass4Web.fetch('group', {
            method: 'POST',
            data: { parent_id: this.props.group.id, title: name },
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

    // ── Delete entry ─────────────────────────────────────────────

    deleteEntry(entry, e) {
        e.stopPropagation()
        if (!window.confirm(`Delete "${entry.title}"?`)) return
        KeePass4Web.fetch('entry', {
            method: 'DELETE',
            data: { id: entry.id },
            success: () => { if (this.props.onEntryDeleted) this.props.onEntryDeleted() },
            error: KeePass4Web.error.bind(this),
        })
    }

    // ── Delete group ─────────────────────────────────────────────

    deleteGroup() {
        const { group } = this.props
        const entries = (group.entries || []).length
        const warning = entries > 0
            ? `Delete "${group.title}" and the ${entries} entr${entries !== 1 ? 'ies' : 'y'} in it?`
            : `Delete "${group.title}"?`
        if (!window.confirm(warning)) return

        KeePass4Web.fetch('group', {
            method: 'DELETE',
            data: { id: group.id },
            success: () => { if (this.props.onGroupDeleted) this.props.onGroupDeleted() },
            error: KeePass4Web.error.bind(this),
        })
    }

    // ── Move ─────────────────────────────────────────────────────

    moveEntry(entry, e) {
        e.stopPropagation()
        this.setState({ moving: { kind: 'entry', id: entry.id, title: entry.title } })
    }

    moveGroup() {
        const { group } = this.props
        this.setState({ moving: { kind: 'group', id: group.id, title: group.title } })
    }

    confirmMove(groupId) {
        const { moving } = this.state
        if (!moving) return

        KeePass4Web.fetch(moving.kind === 'entry' ? 'move_entry' : 'move_group', {
            method: 'PUT',
            data: { id: moving.id, group_id: groupId },
            success: () => {
                this.setState({ moving: null })
                if (this.props.onMoved) this.props.onMoved()
            },
            error: (err) => {
                this.setState({ moving: null })
                KeePass4Web.error.call(this, err)
            },
        })
    }

    render() {
        const { group, groupDepth, selectedEntryId, onSelect, onNewEntry, onEditEntry, mask } = this.props
        const { editingTitle, titleDraft, titleSaving, showGroupForm, newGroupName, groupSaving, moving } = this.state

        const panelClass = `kp-center${mask ? ' kp-loading' : ''}`
        if (!group) return <div className="kp-center"/>

        const isSearch    = group.id === NIL_UUID
        const canAddGroup = !isSearch
        // the root has no parent to be moved to and cannot be deleted
        const canEditGroup = !isSearch && groupDepth !== undefined && groupDepth > 0

        // ── heading ──────────────────────────────────────────────
        let titleEl
        if (editingTitle) {
            titleEl = (
                <form className="kp-center-title-form" onSubmit={this.saveRename.bind(this)}>
                    <input
                        className="kp-input"
                        value={titleDraft}
                        autoFocus
                        style={{ maxWidth: 200, padding: '4px 8px', fontSize: 15, fontWeight: 600 }}
                        onChange={e => this.setState({ titleDraft: e.target.value })}
                    />
                    <button type="submit" className="kp-btn-icon" title="Save" disabled={titleSaving}>
                        <IconCheck size={14}/>
                    </button>
                    <button type="button" className="kp-btn-icon" title="Cancel"
                            onClick={() => this.setState({ editingTitle: false })}>
                        <IconX size={14}/>
                    </button>
                </form>
            )
        } else {
            titleEl = (
                <div className="kp-center-title">
                    <h2 data-testid="group-title">
                        {group.title}
                        {!isSearch && (
                            <button
                                className="kp-btn-link"
                                title="Rename group"
                                onClick={this.startRename.bind(this)}
                                style={{ marginLeft: 6 }}
                            >
                                <IconPencil size={13}/>
                            </button>
                        )}
                        {canEditGroup && (
                            <button
                                className="kp-btn-link"
                                title="Move group"
                                data-testid="move-group"
                                onClick={this.moveGroup.bind(this)}
                                style={{ marginLeft: 4 }}
                            >
                                <IconMove size={13}/>
                            </button>
                        )}
                        {canEditGroup && (
                            <button
                                className="kp-btn-link"
                                title="Delete group"
                                data-testid="delete-group"
                                onClick={this.deleteGroup.bind(this)}
                                style={{ marginLeft: 4 }}
                            >
                                <IconTrash size={13}/>
                            </button>
                        )}
                    </h2>
                    <small>
                        {(group.entries || []).length} saved entr{(group.entries || []).length !== 1 ? 'ies' : 'y'}
                    </small>
                </div>
            )
        }

        // ── entry cards ──────────────────────────────────────────
        const cards = (group.entries || []).map(entry => (
            <div
                key={entry.id}
                className={`kp-card${selectedEntryId === entry.id ? ' active' : ''}`
                    + (isExpired(entry) ? ' expired' : '')}
                data-testid="entry-card"
                onClick={() => onSelect && onSelect(entry)}
            >
                <div className="kp-card-icon">
                    {entryIcon(entry)}
                </div>
                <div className="kp-card-body">
                    <div className="kp-card-title" data-testid="entry-card-title">{entry.title}</div>
                    <div className="kp-card-meta" data-testid="entry-card-meta">
                        {[entry.username, entry.url].filter(Boolean).join(' · ')}
                    </div>
                </div>
                <div className="kp-card-actions">
                    <button
                        className="kp-btn-icon"
                        title="Edit entry"
                        onClick={ev => { ev.stopPropagation(); onEditEntry && onEditEntry(entry) }}
                    >
                        <IconPencil size={13}/>
                    </button>
                    <button
                        className="kp-btn-icon"
                        title="Move entry"
                        data-testid="move-entry"
                        onClick={this.moveEntry.bind(this, entry)}
                    >
                        <IconMove size={13}/>
                    </button>
                    <button
                        className="kp-btn-icon"
                        title="Delete entry"
                        style={{ color: 'var(--kp-text-muted)' }}
                        onClick={this.deleteEntry.bind(this, entry)}
                    >
                        <IconTrash size={13}/>
                    </button>
                </div>
            </div>
        ))

        return (
            <div className={panelClass}>
                <div className="kp-center-header">
                    {titleEl}

                    {!editingTitle && (
                        <div className="kp-center-actions">
                            {group.entries && group.entries.length > 0 && (
                                <span className="kp-badge kp-badge-protected">
                                    <IconLock size={11}/>
                                    protected
                                </span>
                            )}
                            {canAddGroup && (
                                <button
                                    className="kp-btn kp-btn-ghost kp-btn-sm"
                                    onClick={() => this.setState({ showGroupForm: true, newGroupName: '' })}
                                    title="Add subgroup"
                                >
                                    <IconFolder size={13}/>
                                    Add Group
                                </button>
                            )}
                            {!isSearch && (
                                <button
                                    className="kp-btn kp-btn-primary kp-btn-sm"
                                    onClick={() => onNewEntry && onNewEntry()}
                                >
                                    <IconPlus size={13}/>
                                    New Entry
                                </button>
                            )}
                        </div>
                    )}
                </div>

                {/* Add-group form */}
                {showGroupForm && (
                    <div className="kp-group-form">
                        <form onSubmit={this.saveGroup.bind(this)}>
                            <input
                                className="kp-input"
                                placeholder="Group name"
                                required autoFocus
                                value={newGroupName}
                                style={{ maxWidth: 260 }}
                                onChange={e => this.setState({ newGroupName: e.target.value })}
                            />
                            <button type="submit" className="kp-btn kp-btn-primary kp-btn-sm" disabled={groupSaving}>
                                {groupSaving ? '…' : 'Create'}
                            </button>
                            <button type="button" className="kp-btn kp-btn-outline kp-btn-sm"
                                    onClick={() => this.setState({ showGroupForm: false })}>
                                Cancel
                            </button>
                        </form>
                    </div>
                )}

                <div className="kp-entries">
                    {cards.length > 0 ? cards : (
                        <div style={{ padding: '40px 0', textAlign: 'center', color: 'var(--kp-text-muted)', fontSize: 13 }}>
                            No entries yet. Click <strong>+ New Entry</strong> to add one.
                        </div>
                    )}
                </div>

                {moving && this.props.tree && (
                    <GroupPicker
                        tree={this.props.tree}
                        title={`Move "${moving.title}" to`}
                        currentId={moving.kind === 'entry' ? group.id : undefined}
                        excludeId={moving.kind === 'group' ? moving.id : undefined}
                        onPick={this.confirmMove.bind(this)}
                        onCancel={() => this.setState({ moving: null })}
                    />
                )}
            </div>
        )
    }
}

export default withNavigateHook(GroupViewer)
