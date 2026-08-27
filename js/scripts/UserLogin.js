import React from 'react'
import LoginForm from './LoginForm'
import NavBar from './NavBar'
import Alert from './Alert'
import Info from './Info'
import withNavigateHook from './nagivateHook'
import { IconShield, IconKey, IconUser } from './Icons'

class UserLogin extends LoginForm {
    constructor() {
        super()
        this.url = 'user_login'
        this.formRefs.username = React.createRef()
        this.formRefs.password = React.createRef()
    }

    componentDidMount() {
        if (this.props.location.state && this.props.location.state.no_login)
            this.handleLogin()
    }

    render() {
        return (
            <div>
                <NavBar/>
                <div className="container">
                    <div className={this.classes()}>
                        <form className="kp-login-inner" onSubmit={this.handleLogin}>
                            <div className="kp-login-icon">
                                <IconShield size={28}/>
                            </div>
                            <h4>Sign In</h4>
                            <p className="kp-login-sub">Enter your credentials to continue</p>

                            <div className="kp-login-field">
                                <label htmlFor="kp-ul-username">
                                    <IconUser size={13}/>
                                    Username
                                </label>
                                <input
                                    id="kp-ul-username"
                                    className="kp-input"
                                    autoComplete="on"
                                    type="text"
                                    ref={this.formRefs.username}
                                    placeholder="Username"
                                    required
                                    autoFocus={!this.state.error}
                                />
                            </div>

                            <div className="kp-login-field">
                                <label htmlFor="kp-ul-password">
                                    <IconKey size={13}/>
                                    Password
                                </label>
                                <input
                                    id="kp-ul-password"
                                    className="kp-input"
                                    type="password"
                                    ref={this.formRefs.password}
                                    placeholder="Password"
                                    required
                                    autoFocus={!!this.state.error}
                                />
                            </div>

                            <button className="kp-btn kp-btn-primary kp-btn-open" type="submit">
                                Sign In
                            </button>

                            <Alert error={this.state.error}/>
                            <Info info={this.props.location.state && this.props.location.state.info}/>
                        </form>
                    </div>
                </div>
            </div>
        )
    }
}

export default withNavigateHook(UserLogin)
