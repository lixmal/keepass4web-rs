import React from 'react'
import LoginForm from './LoginForm'
import NavBar from './NavBar'
import Alert from './Alert'
import Info from './Info'
import withNavigateHook from './nagivateHook'
import { IconShield, IconKey, IconUser } from './Icons'

class BackendLogin extends LoginForm {
    constructor() {
        super()
        this.url = 'backend_login'
    }

    render() {
        const tpl = KeePass4Web.getSettings().template

        if (!tpl)
            return null

        const fields = []
        for (const i in tpl.fields) {
            const field = tpl.fields[i]
            const isPassword = field.type === 'password'
            fields.push(
                <div className="kp-login-field" key={field.field}>
                    <label htmlFor={`kp-bl-${field.field}`}>
                        {isPassword ? <IconKey size={13}/> : <IconUser size={13}/>}
                        {field.placeholder || field.field}
                    </label>
                    <input
                        id={`kp-bl-${field.field}`}
                        className="kp-input"
                        type={field.type}
                        ref={field.field}
                        placeholder={field.placeholder || ''}
                        required={!!field.required}
                        autoFocus={!!field.autofocus}
                    />
                </div>
            )
        }

        return (
            <div>
                <NavBar/>
                <div className="container">
                    <div className={this.classes()}>
                        <form className="kp-login-inner" onSubmit={this.handleLogin}>
                            <div className="kp-login-icon">
                                {tpl.icon_src
                                    ? <img src={tpl.icon_src} style={{ width: 28, height: 28, objectFit: 'contain' }} alt=""/>
                                    : <IconShield size={28}/>
                                }
                            </div>
                            <h4>{tpl.login_title || 'Sign In'}</h4>
                            <p className="kp-login-sub">Authenticate to access the vault</p>

                            {fields}

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

export default withNavigateHook(BackendLogin)
