import React from 'react'
import { Link } from 'react-router'
import Timer from './Timer'
import withNavigateHook from './nagivateHook'
import {
    IconShield, IconSearch, IconUser, IconChevDown,
    IconSync, IconSave, IconX, IconMenu, IconMoon, IconSun,
} from './Icons'

function getTheme() {
    return localStorage.getItem('kp-theme') || 'light'
}
function applyTheme(t) {
    document.documentElement.setAttribute('data-theme', t)
    localStorage.setItem('kp-theme', t)
}

class NavBar extends React.Component {
    constructor(props) {
        super(props)
        this.state = { dropdownOpen: false, theme: getTheme() }
        this.onLogout   = this.onLogout.bind(this)
        this.onCloseDB  = this.onCloseDB.bind(this)
        this.onTimeUp   = this.onTimeUp.bind(this)
        this.closeDropdown = this.closeDropdown.bind(this)
    }

    onLogout() {
        this.serverRequest = KeePass4Web.fetch('logout', {
            success: (data) => {
                KeePass4Web.clearStorage()
                if (data && data.type === 'redirect' && data.url) {
                    window.location = data.url
                } else {
                    this.props.navigate('/', { replace: true })
                }
            },
            error: KeePass4Web.error.bind(this),
        })
    }

    onCloseDB(e, state) {
        this.serverRequest = KeePass4Web.fetch('close_db', {
            success: () => this.props.navigate('/', { state, replace: true }),
            error: KeePass4Web.error.bind(this),
        })
    }

    onTimeUp() {
        this.onCloseDB(null, { info: 'Database session expired' })
    }

    closeDropdown(e) {
        if (!e.target.closest('.kp-dropdown')) {
            this.setState({ dropdownOpen: false })
        }
    }

    toggleTheme() {
        const next = this.state.theme === 'light' ? 'dark' : 'light'
        applyTheme(next)
        this.setState({ theme: next })
    }

    componentDidMount() {
        applyTheme(this.state.theme)
        document.addEventListener('click', this.closeDropdown)
    }

    componentWillUnmount() {
        document.removeEventListener('click', this.closeDropdown)
        if (this.serverRequest) this.serverRequest.abort()
    }

    render() {
        const cn = KeePass4Web.getSettings().cn
        const loc = this.props.location
        const isVault = loc && !['/backend_login', '/db_login', '/user_login'].includes(loc.pathname)

        // Only render the full vault nav when logged in and past login screens
        if (!cn) {
            return (
                <nav className="kp-nav">
                    <Link className="kp-nav-brand" to="/" replace>
                        <IconShield size={20}/>
                        KeePass 4 Web
                    </Link>
                    <div className="kp-nav-spacer"/>
                    <Link className="kp-btn kp-btn-ghost" to="/" replace>Login</Link>
                </nav>
            )
        }

        // Session timer
        let timer = null
        const timeout = KeePass4Web.getSettings().timeout
        if (timeout && this.props.showSearch) {
            timer = (
                <span className="kp-nav-status" data-testid="db-timer" style={{ gap: 8 }}>
                    <Timer
                        format='{hh}:{mm}:{ss}'
                        timeout={timeout}
                        onTimeUp={this.onTimeUp}
                        restart={KeePass4Web.restartTimer}
                    />
                    <button
                        className="kp-btn-icon"
                        data-testid="db-timer-reset"
                        style={{ padding: '4px 6px' }}
                        onClick={() => KeePass4Web.restartTimer(true)}
                        title="Reset timer"
                    >
                        <IconSync size={13}/>
                    </button>
                </span>
            )
        }

        // Search box
        let search = null
        if (this.props.showSearch) {
            search = (
                <form
                    className="kp-nav-search"
                    role="search"
                    onSubmit={e => this.props.onSearch && this.props.onSearch(this.searchRef, e)}
                >
                    <IconSearch size={14}/>
                    <input
                        type="search"
                        placeholder="Search entries, usernames, URLs…"
                        autoComplete="on"
                        ref={r => (this.searchRef = { term: r })}
                    />
                </form>
            )
        }

        // Mobile sidebar toggle
        const sidebarToggle = this.props.onToggleSidebar ? (
            <button
                className="kp-btn kp-btn-ghost kp-sidebar-toggle"
                style={{ display: 'none', padding: '7px 8px' }}
                onClick={this.props.onToggleSidebar}
                aria-label="Toggle sidebar"
            >
                <IconMenu size={18}/>
            </button>
        ) : null

        return (
            <nav className="kp-nav">
                {sidebarToggle}

                <Link className="kp-nav-brand" to="/" replace>
                    <IconShield size={20}/>
                    KeePass 4 Web
                </Link>

                {cn && isVault && (
                    <span className="kp-nav-status">
                        Vault unlocked
                    </span>
                )}

                {search}
                {timer}

                <div className="kp-nav-spacer"/>

                <button
                    className="kp-theme-toggle"
                    onClick={this.toggleTheme.bind(this)}
                    title={this.state.theme === 'dark' ? 'Switch to light theme' : 'Switch to dark theme'}
                >
                    {this.state.theme === 'dark' ? <IconSun size={15}/> : <IconMoon size={15}/>}
                </button>

                <div className="kp-nav-actions">
                    <div className={`kp-dropdown${this.state.dropdownOpen ? ' open' : ''}`}>
                        <button
                            className="kp-btn kp-btn-ghost"
                            data-testid="user-menu"
                            onClick={() => this.setState(p => ({ dropdownOpen: !p.dropdownOpen }))}
                        >
                            <IconUser size={14}/>
                            {cn}
                            <IconChevDown size={10}/>
                        </button>
                        <div className="kp-dropdown-menu" data-testid="user-menu-list">
                            <button id="logout" className="kp-dropdown-item" onClick={this.onLogout}>
                                Logout
                            </button>
                            <div className="kp-dropdown-divider"/>
                            <button
                                id="closeDB"
                                className="kp-dropdown-item danger"
                                onClick={this.onCloseDB}
                                style={isVault ? {} : { display: 'none' }}
                            >
                                Close Vault
                            </button>
                        </div>
                    </div>

                    {this.props.onSaveDb && (
                        <button
                            className="kp-btn kp-btn-primary"
                            onClick={this.props.onSaveDb}
                        >
                            <IconSave size={14}/>
                            Save
                        </button>
                    )}
                </div>
            </nav>
        )
    }
}

export default withNavigateHook(NavBar)
