import React from 'react'
import {Link} from "react-router-dom"
import Timer from './Timer'
import withNavigateHook from './nagivateHook'

window.$ = window.jQuery = require('jquery')
const Bootstrap = require('bootstrap')

class NavBar extends React.Component {
    constructor(props) {
        super(props)
        this.onLogout = this.onLogout.bind(this)
        this.onCloseDB = this.onCloseDB.bind(this)
        this.onTimeUp = this.onTimeUp.bind(this)
    }

    onLogout() {
        this.serverRequest = KeePass4Web.ajax('logout', {
            success: function (data) {
                KeePass4Web.clearStorage()
                data = data && data.data
                if (data && data.type === 'redirect' && data.url) {
                    window.location = data.url
                } else {
                    this.props.navigate('/user_login', {replace: true})
                    this.props.navigate('/', {replace: true})
                }
            }.bind(this),
            error: KeePass4Web.error.bind(this),
        })
    }

    onCloseDB(event, state) {
        this.serverRequest = KeePass4Web.ajax('close_db', {
            success: function () {
                // redirect to home, so checks for proper login can be made

                if (state === undefined)
                    state = {}
                // we haven't changed page, so need a workaround
                state.replace = true
                this.props.navigate('/db_login', {replace: true})
                this.props.navigate('/', state)
            }.bind(this),
            error: KeePass4Web.error.bind(this),
        })
    }

    onTimeUp() {
        this.onCloseDB(null, {
            info: 'Database session expired'
        })
    }

    componentDidMount() {
        if (KeePass4Web.getSettings().cn) {
            document.getElementById('logout').addEventListener('click', this.onLogout)
            document.getElementById('closeDB').addEventListener('click', this.onCloseDB)
        }
    }

    componentWillUnmount() {
        if (this.serverRequest)
            this.serverRequest.abort()
    }

    render() {
        let cn = KeePass4Web.getSettings().cn;
        let dropdown, search, timer;
        if (cn) {
            dropdown = (
                <ul className="dropdown-menu">
                    <li><a id="logout">Logout</a></li>
                    <li role="separator" className="divider"></li>
                    <li><a id="closeDB">Close Database</a></li>
                </ul>
            )
        } else {
            cn = 'Not logged in'
            dropdown = (
                <ul className="dropdown-menu">
                    <li><Link to="/" replace>Login</Link></li>
                </ul>
            )
        }

        if (this.props.showSearch) {
            search = (
                <form className="navbar-form navbar-left" role="search"
                      onSubmit={this.props.onSearch.bind(this, this.refs)}>
                    <div className="input-group">
                        <input autoComplete="on" type="search" ref="term" className="form-control" placeholder="Search"
                               autoFocus/>
                        <div className="input-group-btn">
                            <button type="submit" className="btn btn-default"><span
                                className="glyphicon glyphicon-search"></span></button>
                        </div>
                    </div>
                </form>
            )
            let timeout = KeePass4Web.getSettings().timeout
            if (timeout) {
                timer = (
                    <div className="navbar-text">
                        <Timer
                            format='{hh}:{mm}:{ss}'
                            timeout={timeout}
                            onTimeUp={this.onTimeUp}
                            restart={KeePass4Web.restartTimer}
                        />
                        <label type="button" className="btn btn-secondary btn-xs"
                               onClick={KeePass4Web.restartTimer.bind(this, true)}>
                            <span className="glyphicon glyphicon-repeat"></span>
                        </label>
                    </div>
                )
            }
        }

        return (
            <nav className="navbar navbar-default navbar-fixed-top">
                <div className="navbar-header">
                    <button type="button" className="navbar-toggle collapsed" data-toggle="collapse"
                            data-target="#navbar-collapse-1" aria-expanded="false">
                        <span className="sr-only">Toggle navigation</span>
                        <span className="icon-bar"></span>
                        <span className="icon-bar"></span>
                        <span className="icon-bar"></span>
                    </button>
                    <Link className="navbar-brand" to="/" replace>KeePass 4 Web</Link>
                    {timer}
                </div>
                <div className="collapse navbar-collapse" id="navbar-collapse-1">
                    {search}
                    <ul className="nav navbar-nav navbar-right">
                        <li className="dropdown">
                            <a href="" className="dropdown-toggle" data-toggle="dropdown" role="button"
                               aria-haspopup="true" aria-expanded="false">
                                {cn}
                                <span className="caret"></span>
                            </a>
                            {dropdown}
                        </li>
                    </ul>
                </div>
            </nav>
        )
    }
}

export default withNavigateHook(NavBar)
